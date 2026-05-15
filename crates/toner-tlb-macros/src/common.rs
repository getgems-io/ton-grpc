use darling::{FromDeriveInput, FromField, FromVariant, util::SpannedValue};
use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Data, DataEnum, DataStruct, DeriveInput, Expr, Field, Fields, GenericParam, Generics, Ident,
    Result, Type, Variant, parse_quote, punctuated::Punctuated, spanned::Spanned, token::Comma,
};

pub trait Backend {
    fn reader_ident() -> Ident;
    fn impl_block(name: &Ident, generics: &Generics, body: TokenStream) -> TokenStream;
    fn validate_field_mode(kind: &FieldModeKind, span: Span) -> Result<()>;
    fn validate_separate_cell_marker(marker: SeparateCellMarker, span: Span) -> Result<()>;
    fn validate_container_ensure_empty(ensure_empty: bool, span: Span) -> Result<()>;
    fn default_mode_kind() -> FieldModeKind;
}

#[derive(Default)]
pub struct ContainerAttrs {
    pub tag: Option<TagValue>,
    pub ensure_empty: Option<Span>,
}

#[derive(FromDeriveInput)]
#[darling(attributes(tlb), supports(struct_any, enum_any))]
struct RawContainer {
    tag: Option<SpannedValue<String>>,
    ensure_empty: darling::util::Flag,
}

#[derive(FromVariant)]
#[darling(attributes(tlb))]
struct RawVariant {
    tag: Option<SpannedValue<String>>,
}

#[derive(FromField)]
#[darling(attributes(tlb))]
struct RawField {
    ident: Option<Ident>,
    parse: darling::util::Flag,
    parse_as: Option<Type>,
    unpack: darling::util::Flag,
    unpack_as: Option<Type>,
    args: Option<Expr>,
    separate_cell_start: darling::util::Flag,
    separate_cell_end: darling::util::Flag,
}

pub fn parse_container_attrs(input: &DeriveInput) -> Result<ContainerAttrs> {
    let raw = RawContainer::from_derive_input(input)?;
    let tag = raw
        .tag
        .map(|s| TagValue::parse_str(&s, s.span()))
        .transpose()?;
    let ensure_empty = raw
        .ensure_empty
        .is_present()
        .then(|| raw.ensure_empty.span());
    Ok(ContainerAttrs { tag, ensure_empty })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TagFormat {
    Binary,
    Hex,
}

pub struct TagValue {
    bits: Vec<bool>,
    format: TagFormat,
    span: Span,
}

impl TagValue {
    pub fn parse_str(s: &str, span: Span) -> Result<Self> {
        let (rest, format) = match s.as_bytes() {
            [b'0', b'b', ..] => (&s[2..], TagFormat::Binary),
            [b'$', ..] => (&s[1..], TagFormat::Binary),
            [b'0', b'x', ..] => (&s[2..], TagFormat::Hex),
            [b'#', ..] => (&s[1..], TagFormat::Hex),
            _ => return Err(syn::Error::new(span, "tag must start with 0b, 0x, # or $")),
        };
        let bits = match format {
            TagFormat::Binary => rest
                .chars()
                .map(|c| match c {
                    '0' => Ok(false),
                    '1' => Ok(true),
                    other => Err(syn::Error::new(
                        span,
                        format!("invalid binary digit: {other}"),
                    )),
                })
                .collect::<Result<Vec<bool>>>()?,
            TagFormat::Hex => {
                let val = u64::from_str_radix(rest, 16)
                    .map_err(|e| syn::Error::new(span, format!("invalid hex: {e}")))?;
                (0..rest.len() * 4)
                    .rev()
                    .map(|i| (val >> i) & 1 == 1)
                    .collect()
            }
        };
        Ok(Self { bits, format, span })
    }

    pub fn bit_len(&self) -> usize {
        self.bits.len()
    }
    pub fn format(&self) -> TagFormat {
        self.format
    }
    pub fn bits(&self) -> &[bool] {
        &self.bits
    }
    pub fn span(&self) -> Span {
        self.span
    }

    fn as_u64(&self) -> u64 {
        self.bits.iter().fold(0u64, |a, &b| (a << 1) | b as u64)
    }

    pub fn literal(&self) -> Literal {
        format_bits_literal(self.as_u64(), self.bit_len(), self.format)
    }
}

fn format_bits_literal(val: u64, bit_len: usize, format: TagFormat) -> Literal {
    let s = match format {
        TagFormat::Binary => format!("0b{val:0>bit_len$b}", bit_len = bit_len),
        TagFormat::Hex => format!("0x{val:0>hex_digits$x}", hex_digits = bit_len.div_ceil(4)),
    };
    s.parse()
        .expect("formatted bits literal must be a valid Rust literal")
}

pub fn tag_int_type(bit_len: usize, span: Span) -> Result<TokenStream> {
    let ty = match bit_len {
        0..=8 => quote! { u8 },
        9..=16 => quote! { u16 },
        17..=32 => quote! { u32 },
        _ => {
            return Err(syn::Error::new(
                span,
                format!("tags longer than 32 bits are not supported (got {bit_len})"),
            ));
        }
    };
    Ok(ty)
}

pub struct FieldMode {
    pub kind: FieldModeKind,
    pub args: Option<Expr>,
}

pub enum FieldModeKind {
    Parse,
    ParseAs(Type),
    Unpack,
    UnpackAs(Type),
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum SeparateCellMarker {
    #[default]
    None,
    Start,
    End,
    Both,
}

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

    let mode_kind = match (
        raw.parse.is_present(),
        raw.parse_as,
        raw.unpack.is_present(),
        raw.unpack_as,
    ) {
        (false, None, false, None) => None,
        (true, None, false, None) => Some(FieldModeKind::Parse),
        (false, Some(ty), false, None) => Some(FieldModeKind::ParseAs(ty)),
        (false, None, true, None) => Some(FieldModeKind::Unpack),
        (false, None, false, Some(ty)) => Some(FieldModeKind::UnpackAs(ty)),
        _ => {
            return Err(syn::Error::new(
                span,
                "at most one of `parse`, `parse_as`, `unpack`, `unpack_as` may be specified",
            ));
        }
    };
    let mode = mode_kind
        .map(|kind| FieldMode {
            kind,
            args: raw.args,
        })
        .unwrap_or(FieldMode {
            kind: B::default_mode_kind(),
            args: None,
        });

    let marker = match (
        raw.separate_cell_start.is_present(),
        raw.separate_cell_end.is_present(),
    ) {
        (false, false) => SeparateCellMarker::None,
        (true, false) => SeparateCellMarker::Start,
        (false, true) => SeparateCellMarker::End,
        (true, true) => SeparateCellMarker::Both,
    };

    B::validate_field_mode(&mode.kind, span)?;
    B::validate_separate_cell_marker(marker, span)?;

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
    let call = match &mode.kind {
        FieldModeKind::Parse => quote! { #reader.parse(#args) },
        FieldModeKind::ParseAs(ty) => quote! { #reader.parse_as::<_, #ty>(#args) },
        FieldModeKind::Unpack => quote! { #reader.unpack(#args) },
        FieldModeKind::UnpackAs(ty) => quote! { #reader.unpack_as::<_, #ty>(#args) },
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

struct VariantInfo {
    ident: Ident,
    tag: TagValue,
    fields: Punctuated<Field, Comma>,
}

fn parse_variant(variant: &Variant) -> Result<VariantInfo> {
    let raw = RawVariant::from_variant(variant)?;
    let tag_str = raw.tag.ok_or_else(|| {
        syn::Error::new_spanned(
            &variant.ident,
            "enum variant must have #[tlb(tag = \"...\")]",
        )
    })?;
    let tag = TagValue::parse_str(&tag_str, tag_str.span())?;
    let fields = match &variant.fields {
        Fields::Named(f) => f.named.clone(),
        Fields::Unit => Punctuated::new(),
        Fields::Unnamed(_) => {
            return Err(syn::Error::new_spanned(
                &variant.ident,
                "tuple variants are not supported",
            ));
        }
    };
    Ok(VariantInfo {
        ident: variant.ident.clone(),
        tag,
        fields,
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

fn wrap_impl<B: Backend>(
    input: &DeriveInput,
    attrs: &ContainerAttrs,
    inner: TokenStream,
) -> TokenStream {
    let name = &input.ident;
    let reader = B::reader_ident();
    let ensure_empty = attrs
        .ensure_empty
        .map(|_| quote! { #reader.ensure_empty()?; });
    let body = quote! {
        use toner::tlb::bits::de::BitReaderExt;
        use toner::tlb::Context;
        #inner
        #ensure_empty
        Ok(__result)
    };
    B::impl_block(name, &input.generics, body)
}

fn parse_and_validate_attrs<B: Backend>(input: &DeriveInput) -> Result<ContainerAttrs> {
    let attrs = parse_container_attrs(input)?;
    B::validate_container_ensure_empty(
        attrs.ensure_empty.is_some(),
        attrs.ensure_empty.unwrap_or_else(|| input.ident.span()),
    )?;
    Ok(attrs)
}

fn expand_struct<B: Backend>(input: &DeriveInput, data: &DataStruct) -> Result<TokenStream> {
    let attrs = parse_and_validate_attrs::<B>(input)?;
    let reader = B::reader_ident();

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
        &attrs,
        quote! {
            #tag_stmt
            #stmts
            let __result = #constructor;
        },
    ))
}

fn expand_enum<B: Backend>(input: &DeriveInput, data: &DataEnum) -> Result<TokenStream> {
    let attrs = parse_and_validate_attrs::<B>(input)?;
    let reader = B::reader_ident();
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
    Ok(wrap_impl::<B>(input, &attrs, inner))
}

pub fn extend_generics_with_de(generics: &Generics) -> (TokenStream, TokenStream, TokenStream) {
    let mut extended = generics.clone();
    extended
        .params
        .insert(0, GenericParam::Lifetime(parse_quote!('de)));
    let (impl_g, _, _) = extended.split_for_impl();
    let (_, ty_g, where_g) = generics.split_for_impl();
    (quote! { #impl_g }, quote! { #ty_g }, quote! { #where_g })
}
