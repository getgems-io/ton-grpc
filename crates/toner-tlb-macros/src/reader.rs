use darling::FromField;
use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Data, DataEnum, DataStruct, DeriveInput, Field, Fields, Ident, Result, punctuated::Punctuated,
    spanned::Spanned, token::Comma,
};

use crate::common::{
    Backend, FieldLayer, FieldMode, RawField, SeparateCellMarker, TagFormat, TagValue, VariantInfo,
    format_bits_literal, parse_container_attrs, parse_variant, tag_int_type,
};

pub struct FieldEntry {
    pub binding: Ident,
    pub mode: FieldMode,
    pub context: String,
    pub separate_cell: SeparateCellMarker,
    pub span: Span,
}

fn build_entries<B: Backend>(
    fields: &Punctuated<Field, Comma>,
    named: bool,
) -> Result<Vec<FieldEntry>> {
    fields
        .iter()
        .enumerate()
        .map(|(i, f)| build_entry::<B>(f, i, named))
        .collect()
}

fn build_entry<B: Backend>(f: &Field, index: usize, named: bool) -> Result<FieldEntry> {
    let raw = RawField::from_field(f)?;
    let span = f.span();
    let (binding, context) = if named {
        let id = raw.ident.clone().expect("named field must have ident");
        (id.clone(), id.to_string())
    } else {
        (format_ident!("__field_{index}"), index.to_string())
    };

    let marker = match (
        raw.separate_cell_start.is_present(),
        raw.separate_cell_end.is_present(),
    ) {
        (false, false) => SeparateCellMarker::None,
        (true, false) => SeparateCellMarker::Start,
        (false, true) => SeparateCellMarker::End,
        (true, true) => SeparateCellMarker::Both,
    };
    B::validate_separate_cell_marker(marker, span)?;

    let mode = FieldMode::from_raw::<B>(raw, span)?;

    Ok(FieldEntry {
        binding,
        mode,
        context,
        separate_cell: marker,
        span,
    })
}

/// Linear codegen for a sequence of fields where `^[ ... ]` (TLB "separate cell")
/// blocks are inlined: each block opens a fresh child-cell parser, fields inside
/// are read from it, and the block must be fully consumed at its end.
fn gen_field_stmts(reader: &Ident, entries: &[FieldEntry]) -> Result<TokenStream> {
    let sub = format_ident!("__separate_cell_parser");
    let mut out = TokenStream::new();
    let mut block_open_span: Option<Span> = None;

    for entry in entries {
        let span = entry.span;
        let (opens, closes) = match entry.separate_cell {
            SeparateCellMarker::None => (false, false),
            SeparateCellMarker::Start => (true, false),
            SeparateCellMarker::End => (false, true),
            SeparateCellMarker::Both => (true, true),
        };
        let inside_block = block_open_span.is_some();

        match (inside_block, opens, closes) {
            (true, true, _) => {
                return Err(syn::Error::new(
                    span,
                    "nested `separate_cell_start` is not allowed; close the previous block with `separate_cell_end` first",
                ));
            }
            (false, false, true) => {
                return Err(syn::Error::new(
                    span,
                    "`separate_cell_end` without a preceding `separate_cell_start`",
                ));
            }
            _ => {}
        }

        if opens {
            out.extend(quote! {
                let mut #sub: toner::tlb::de::CellParser<'de> = #reader
                    .parse_as::<toner::tlb::de::CellParser<'de>, toner::tlb::Ref>(())
                    .context("^[")?;
            });
            block_open_span = Some(span);
        }

        let active = if block_open_span.is_some() {
            &sub
        } else {
            reader
        };
        out.extend(gen_field_call(active, entry));

        if closes {
            out.extend(quote! { #sub.ensure_empty().context("^]")?; });
            block_open_span = None;
        }
    }

    if let Some(span) = block_open_span {
        return Err(syn::Error::new(
            span,
            "`separate_cell_start` without a matching `separate_cell_end`",
        ));
    }

    Ok(out)
}

fn gen_field_call(reader: &Ident, entry: &FieldEntry) -> TokenStream {
    let FieldEntry {
        binding,
        mode,
        context,
        ..
    } = entry;
    let args = match &mode.args {
        Some(expr) => quote! { #expr },
        None => quote! { () },
    };
    let call = match (mode.layer, &mode.as_ty) {
        (FieldLayer::Cell, None) => quote! { #reader.parse(#args) },
        (FieldLayer::Cell, Some(ty)) => quote! { #reader.parse_as::<_, #ty>(#args) },
        (FieldLayer::Bits, None) => quote! { #reader.unpack(#args) },
        (FieldLayer::Bits, Some(ty)) => quote! { #reader.unpack_as::<_, #ty>(#args) },
    };
    quote! { let #binding = #call.context(#context)?; }
}

fn gen_tag_check(reader: &Ident, tag: &TagValue, type_name: &str) -> Result<TokenStream> {
    let bit_len = tag.bit_len();
    let int_ty = tag_int_type(bit_len, tag.span())?;
    let nbits = Literal::usize_unsuffixed(bit_len);
    let tag_lit = tag.literal();
    let hex_width = bit_len.div_ceil(4);
    let fmt_str = format!("invalid {type_name} tag: 0x{{:0>{hex_width}x}}");

    Ok(quote! {
        let __tag: #int_ty = #reader.unpack_as::<_, toner::tlb::bits::NBits<#nbits>>(())?;
        if __tag != #tag_lit as #int_ty {
            return Err(toner::tlb::Error::custom(format!(#fmt_str, __tag)));
        }
    })
}

fn gen_variant_body<B: Backend>(
    reader: &Ident,
    type_name: &Ident,
    variant: &VariantInfo,
) -> Result<TokenStream> {
    let variant_ident = &variant.ident;
    if variant.fields.is_empty() {
        return Ok(quote! { #type_name::#variant_ident });
    }
    let entries = build_entries::<B>(&variant.fields, true)?;
    let names: Vec<Ident> = entries.iter().map(|e| e.binding.clone()).collect();
    let stmts = gen_field_stmts(reader, &entries)?;
    Ok(quote! {
        {
            #stmts
            #type_name::#variant_ident { #(#names,)* }
        }
    })
}

struct TagTree {
    leaf: Option<usize>,
    zero: Option<Box<TagTree>>,
    one: Option<Box<TagTree>>,
}

impl TagTree {
    fn new() -> Self {
        Self {
            leaf: None,
            zero: None,
            one: None,
        }
    }

    fn insert(&mut self, bits: &[bool], idx: usize) {
        if let Some((head, tail)) = bits.split_first() {
            let child = if *head { &mut self.one } else { &mut self.zero };
            child
                .get_or_insert_with(|| Box::new(TagTree::new()))
                .insert(tail, idx);
        } else {
            self.leaf = Some(idx);
        }
    }

    fn min_leaf_depth(&self) -> usize {
        if self.leaf.is_some() {
            return 0;
        }
        let z = self.zero.as_ref().map(|c| 1 + c.min_leaf_depth());
        let o = self.one.as_ref().map(|c| 1 + c.min_leaf_depth());
        z.into_iter().chain(o).min().unwrap_or(0)
    }

    fn is_pure_leaf(&self) -> Option<usize> {
        if self.zero.is_none() && self.one.is_none() {
            self.leaf
        } else {
            None
        }
    }

    fn children(&self) -> [(bool, Option<&TagTree>); 2] {
        [(false, self.zero.as_deref()), (true, self.one.as_deref())]
    }
}

struct Dispatcher<'a> {
    reader: &'a Ident,
    enum_name: &'a str,
    format: TagFormat,
    bodies: &'a [TokenStream],
}

impl<'a> Dispatcher<'a> {
    fn build(&self, node: &TagTree, depth: usize) -> Result<TokenStream> {
        if let Some(idx) = node.is_pure_leaf() {
            return Ok(self.bodies[idx].clone());
        }

        let read_bits = node.min_leaf_depth();
        let int_ty = tag_int_type(read_bits, Span::call_site())?;
        let tag_ident = format_ident!("__tag_{depth}");
        let nbits = Literal::usize_unsuffixed(read_bits);
        let reader = self.reader;

        let mut arms: Vec<TokenStream> = Vec::new();
        self.collect_arms(
            node,
            depth,
            read_bits,
            read_bits,
            &mut Vec::new(),
            &mut arms,
        )?;

        let err_msg = format!("invalid {} tag", self.enum_name);
        Ok(quote! {
            {
                let #tag_ident: #int_ty = #reader.unpack_as::<_, toner::tlb::bits::NBits<#nbits>>(())?;
                match #tag_ident {
                    #(#arms)*
                    _ => return Err(toner::tlb::Error::custom(
                        format!(concat!(#err_msg, ": {}"), #tag_ident)
                    )),
                }
            }
        })
    }

    fn collect_arms(
        &self,
        node: &TagTree,
        depth: usize,
        remaining: usize,
        total_bits: usize,
        prefix: &mut Vec<bool>,
        arms: &mut Vec<TokenStream>,
    ) -> Result<()> {
        if remaining == 0 {
            let val = prefix.iter().fold(0u64, |a, &b| (a << 1) | b as u64);
            let val_lit = format_bits_literal(val, total_bits, self.format);
            let body = match node.is_pure_leaf() {
                Some(idx) => self.bodies[idx].clone(),
                None => self.build(node, depth + 1)?,
            };
            arms.push(quote! { #val_lit => #body, });
            return Ok(());
        }

        for (bit, child) in node.children() {
            let Some(child) = child else { continue };
            prefix.push(bit);
            self.collect_arms(child, depth, remaining - 1, total_bits, prefix, arms)?;
            prefix.pop();
        }
        Ok(())
    }
}

pub fn expand<B: Backend>(input: DeriveInput) -> Result<TokenStream> {
    match &input.data {
        Data::Struct(data) => expand_struct::<B>(&input, data),
        Data::Enum(data) => expand_enum::<B>(&input, data),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "derive not supported for unions",
        )),
    }
}

fn wrap_impl<B: Backend>(input: &DeriveInput, inner: TokenStream) -> TokenStream {
    let name = &input.ident;
    let body = quote! {
        use toner::tlb::bits::de::BitReaderExt;
        use toner::tlb::Context;
        #inner
        Ok(__result)
    };
    B::impl_block(name, &input.generics, body)
}

fn expand_struct<B: Backend>(input: &DeriveInput, data: &DataStruct) -> Result<TokenStream> {
    let attrs = parse_container_attrs(input)?;
    let reader = B::ident();
    let (named, fields) = match &data.fields {
        Fields::Named(f) => (true, &f.named),
        Fields::Unnamed(f) => (false, &f.unnamed),
        Fields::Unit => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "derive does not support unit structs",
            ));
        }
    };
    let entries = build_entries::<B>(fields, named)?;
    let idents: Vec<Ident> = entries.iter().map(|e| e.binding.clone()).collect();
    let stmts = gen_field_stmts(&reader, &entries)?;
    let constructor = if named {
        quote! { Self { #(#idents,)* } }
    } else {
        quote! { Self(#(#idents,)*) }
    };
    let tag_stmt = attrs
        .tag
        .as_ref()
        .map(|tag| gen_tag_check(&reader, tag, &input.ident.to_string()))
        .transpose()?;

    Ok(wrap_impl::<B>(
        input,
        quote! {
            #tag_stmt
            #stmts
            let __result = #constructor;
        },
    ))
}

fn expand_enum<B: Backend>(input: &DeriveInput, data: &DataEnum) -> Result<TokenStream> {
    let _attrs = parse_container_attrs(input)?;
    let reader = B::ident();
    let name = &input.ident;

    let variants: Vec<VariantInfo> = data
        .variants
        .iter()
        .map(parse_variant)
        .collect::<Result<_>>()?;
    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            name,
            "enum must have at least one variant",
        ));
    }

    let mut tree = TagTree::new();
    for (i, v) in variants.iter().enumerate() {
        tree.insert(v.tag.bits(), i);
    }
    let bodies: Vec<TokenStream> = variants
        .iter()
        .map(|v| gen_variant_body::<B>(&reader, name, v))
        .collect::<Result<_>>()?;
    let dispatch = Dispatcher {
        reader: &reader,
        enum_name: &name.to_string(),
        format: variants[0].tag.format(),
        bodies: &bodies,
    }
    .build(&tree, 0)?;

    let inner = quote! { let __result = #dispatch; };
    Ok(wrap_impl::<B>(input, inner))
}
