use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DataEnum, DataStruct, DeriveInput, Expr, Field, Fields, Ident, LitStr, Result,
    Type, Variant, punctuated::Punctuated, spanned::Spanned, token::Comma,
};

pub trait Backend {
    fn reader_ident() -> Ident;

    fn impl_block(name: &Ident, generics: &syn::Generics, body: TokenStream) -> TokenStream;

    fn validate_field_mode(kind: &FieldModeKind, span: Span) -> Result<()>;

    fn validate_separate_cell_marker(marker: SeparateCellMarker, span: Span) -> Result<()>;

    fn validate_container_ensure_empty(ensure_empty: bool, type_span: Span) -> Result<()>;

    fn default_mode_kind() -> FieldModeKind;
}

#[derive(Default)]
pub struct ContainerAttrs {
    pub tag: Option<TagValue>,
    pub ensure_empty: bool,
    pub ensure_empty_span: Option<Span>,
}

pub fn parse_container_attrs(attrs: &[Attribute]) -> Result<ContainerAttrs> {
    let mut result = ContainerAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("tlb") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                let lit: LitStr = meta.value()?.parse()?;
                result.tag = Some(TagValue::parse_lit(&lit)?);
            } else if meta.path.is_ident("ensure_empty") {
                result.ensure_empty = true;
                result.ensure_empty_span = Some(meta.path.span());
            } else {
                return Err(meta.error("unknown tlb attribute"));
            }
            Ok(())
        })?;
    }
    Ok(result)
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
    pub fn parse_lit(lit: &LitStr) -> Result<Self> {
        let s = lit.value();
        let span = lit.span();
        let (bits, format) =
            if let Some(rest) = s.strip_prefix("0b").or_else(|| s.strip_prefix('$')) {
                (parse_binary(rest, span)?, TagFormat::Binary)
            } else if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix('#')) {
                (parse_hex(rest, span)?, TagFormat::Hex)
            } else {
                return Err(syn::Error::new(span, "tag must start with 0b, 0x, # or $"));
            };
        Ok(Self { bits, format, span })
    }

    pub fn bit_len(&self) -> usize {
        self.bits.len()
    }

    fn as_u64(&self) -> u64 {
        self.bits.iter().fold(0u64, |a, &b| (a << 1) | b as u64)
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
}

fn parse_binary(s: &str, span: Span) -> Result<Vec<bool>> {
    s.chars()
        .map(|c| match c {
            '0' => Ok(false),
            '1' => Ok(true),
            other => Err(syn::Error::new(
                span,
                format!("invalid binary digit: {other}"),
            )),
        })
        .collect()
}

fn parse_hex(s: &str, span: Span) -> Result<Vec<bool>> {
    let val = u64::from_str_radix(s, 16)
        .map_err(|e| syn::Error::new(span, format!("invalid hex: {e}")))?;
    let bit_count = s.len() * 4;
    Ok((0..bit_count).rev().map(|i| (val >> i) & 1 == 1).collect())
}

pub fn tag_literal(tag: &TagValue) -> Literal {
    let val = tag.as_u64();
    let bit_len = tag.bit_len();
    let s = match tag.format {
        TagFormat::Binary => format!("0b{val:0>bit_len$b}", bit_len = bit_len),
        TagFormat::Hex => format!("0x{val:0>hex_digits$x}", hex_digits = bit_len.div_ceil(4)),
    };
    s.parse()
        .expect("formatted tag literal must be a valid Rust literal")
}

pub fn tag_int_type(bit_len: usize, span: Span) -> Result<TokenStream> {
    match bit_len {
        0..=8 => Ok(quote! { u8 }),
        9..=16 => Ok(quote! { u16 }),
        17..=32 => Ok(quote! { u32 }),
        _ => Err(syn::Error::new(
            span,
            format!("tags longer than 32 bits are not supported (got {bit_len})"),
        )),
    }
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

fn parse_field_attrs<B: Backend>(
    attrs: &[Attribute],
    span: Span,
) -> Result<(Option<FieldMode>, SeparateCellMarker)> {
    let mut kind: Option<FieldModeKind> = None;
    let mut args: Option<Expr> = None;
    let mut start = false;
    let mut end = false;

    for attr in attrs {
        if !attr.path().is_ident("tlb") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("parse") {
                if meta.input.peek(syn::Token![=]) {
                    return Err(meta.error("use `parse_as` for typed parse"));
                }
                kind = Some(FieldModeKind::Parse);
            } else if meta.path.is_ident("parse_as") {
                kind = Some(FieldModeKind::ParseAs(parse_as_type(&meta)?));
            } else if meta.path.is_ident("unpack") {
                if meta.input.peek(syn::Token![=]) {
                    return Err(meta.error("use `unpack_as` for typed unpack"));
                }
                kind = Some(FieldModeKind::Unpack);
            } else if meta.path.is_ident("unpack_as") {
                kind = Some(FieldModeKind::UnpackAs(parse_as_type(&meta)?));
            } else if meta.path.is_ident("args") {
                let lit: LitStr = meta.value()?.parse()?;
                args = Some(syn::parse_str(&lit.value())?);
            } else if meta.path.is_ident("separate_cell_start") {
                start = true;
            } else if meta.path.is_ident("separate_cell_end") {
                end = true;
            } else {
                return Err(meta.error("unknown tlb field attribute"));
            }
            Ok(())
        })?;
    }

    let marker = match (start, end) {
        (false, false) => SeparateCellMarker::None,
        (true, false) => SeparateCellMarker::Start,
        (false, true) => SeparateCellMarker::End,
        (true, true) => SeparateCellMarker::Both,
    };

    let mode = kind.map(|kind| FieldMode { kind, args });
    if let Some(m) = &mode {
        B::validate_field_mode(&m.kind, span)?;
    }
    B::validate_separate_cell_marker(marker, span)?;

    Ok((mode, marker))
}

fn parse_as_type(meta: &syn::meta::ParseNestedMeta) -> Result<Type> {
    let lit: LitStr = meta.value()?.parse()?;
    syn::parse_str(&lit.value())
}

fn build_entries<B: Backend>(
    iter: impl IntoIterator<Item = FieldInput>,
) -> Result<Vec<FieldEntry>> {
    iter.into_iter()
        .map(|input| {
            let (mode_opt, marker) = parse_field_attrs::<B>(&input.attrs, input.span)?;
            let mode = mode_opt.unwrap_or(FieldMode {
                kind: B::default_mode_kind(),
                args: None,
            });
            Ok(FieldEntry {
                binding: input.binding,
                mode,
                context: input.context,
                separate_cell: marker,
                span: input.span,
            })
        })
        .collect()
}

struct FieldInput {
    binding: Ident,
    context: String,
    span: Span,
    attrs: Vec<Attribute>,
}

fn named_field_inputs(fields: &Punctuated<Field, Comma>) -> Vec<FieldInput> {
    fields
        .iter()
        .map(|f| {
            let binding = f.ident.clone().expect("named field must have ident");
            let context = binding.to_string();
            FieldInput {
                context,
                binding,
                span: f.span(),
                attrs: f.attrs.clone(),
            }
        })
        .collect()
}

fn tuple_field_inputs(fields: &Punctuated<Field, Comma>) -> Vec<FieldInput> {
    fields
        .iter()
        .enumerate()
        .map(|(i, f)| FieldInput {
            binding: format_ident!("__field_{i}"),
            context: i.to_string(),
            span: f.span(),
            attrs: f.attrs.clone(),
        })
        .collect()
}

/// Models a flat field run or a `^[ ... ]` (TLB "separate cell") block,
/// loaded as a fresh child cell and required to be fully consumed.
pub enum FieldSection {
    Flat(Vec<FieldEntry>),
    SeparateCell(Vec<FieldEntry>),
}

fn group_sections(entries: Vec<FieldEntry>) -> Result<Vec<FieldSection>> {
    let mut sections: Vec<FieldSection> = Vec::new();
    let mut flat: Vec<FieldEntry> = Vec::new();
    let mut open: Option<Vec<FieldEntry>> = None;

    for entry in entries {
        match (open.as_mut(), entry.separate_cell) {
            (None, SeparateCellMarker::None) => flat.push(entry),
            (None, SeparateCellMarker::Start) => {
                if !flat.is_empty() {
                    sections.push(FieldSection::Flat(std::mem::take(&mut flat)));
                }
                open = Some(vec![entry]);
            }
            (None, SeparateCellMarker::Both) => {
                if !flat.is_empty() {
                    sections.push(FieldSection::Flat(std::mem::take(&mut flat)));
                }
                sections.push(FieldSection::SeparateCell(vec![entry]));
            }
            (None, SeparateCellMarker::End) => {
                return Err(syn::Error::new(
                    entry.span,
                    "`separate_cell_end` without a preceding `separate_cell_start`",
                ));
            }
            (Some(_), SeparateCellMarker::Start) => {
                return Err(syn::Error::new(
                    entry.span,
                    "nested `separate_cell_start` is not allowed; close the previous block with `separate_cell_end` first",
                ));
            }
            (Some(_), SeparateCellMarker::Both) => {
                return Err(syn::Error::new(
                    entry.span,
                    "cannot have both `separate_cell_start` and `separate_cell_end` on a field that is already inside an open separate-cell block",
                ));
            }
            (Some(buf), SeparateCellMarker::None) => buf.push(entry),
            (Some(_), SeparateCellMarker::End) => {
                let mut buf = open.take().unwrap();
                buf.push(entry);
                sections.push(FieldSection::SeparateCell(buf));
            }
        }
    }

    if let Some(orphan) = open {
        let span = orphan
            .first()
            .map(|f| f.span)
            .unwrap_or_else(Span::call_site);
        return Err(syn::Error::new(
            span,
            "`separate_cell_start` without a matching `separate_cell_end`",
        ));
    }

    if !flat.is_empty() {
        sections.push(FieldSection::Flat(flat));
    }

    Ok(sections)
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

fn gen_section(reader: &Ident, section: &FieldSection) -> TokenStream {
    match section {
        FieldSection::Flat(entries) => {
            let stmts = entries.iter().map(|e| gen_field_call(reader, e));
            quote! { #(#stmts)* }
        }
        FieldSection::SeparateCell(entries) => {
            let sub = format_ident!("__separate_cell_parser");
            let stmts = entries.iter().map(|e| gen_field_call(&sub, e));
            quote! {
                let mut #sub: toner::tlb::de::CellParser<'de> = #reader
                    .parse_as::<toner::tlb::de::CellParser<'de>, toner::tlb::Ref>(())
                    .context("^[")?;
                #(#stmts)*
                #sub.ensure_empty().context("^]")?;
            }
        }
    }
}

fn gen_tag_check(reader: &Ident, tag: &TagValue, type_name: &str) -> Result<TokenStream> {
    let bit_len = tag.bit_len();
    let int_ty = tag_int_type(bit_len, tag.span())?;
    let nbits = Literal::usize_unsuffixed(bit_len);
    let tag_lit = tag_literal(tag);
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
    let mut tag = None;
    for attr in &variant.attrs {
        if !attr.path().is_ident("tlb") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                let lit: LitStr = meta.value()?.parse()?;
                tag = Some(TagValue::parse_lit(&lit)?);
            } else {
                return Err(meta.error("unknown tlb variant attribute"));
            }
            Ok(())
        })?;
    }
    let tag = tag.ok_or_else(|| {
        syn::Error::new_spanned(
            &variant.ident,
            "enum variant must have #[tlb(tag = \"...\")]",
        )
    })?;
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
    let entries = build_entries::<B>(named_field_inputs(&variant.fields))?;
    let field_names: Vec<Ident> = entries.iter().map(|e| e.binding.clone()).collect();
    let sections = group_sections(entries)?;
    let section_stmts = sections.iter().map(|s| gen_section(reader, s));
    Ok(quote! {
        {
            #(#section_stmts)*
            #type_name::#variant_ident { #(#field_names,)* }
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
        let mut min = usize::MAX;
        if let Some(c) = &self.zero {
            min = min.min(1 + c.min_leaf_depth());
        }
        if let Some(c) = &self.one {
            min = min.min(1 + c.min_leaf_depth());
        }
        if min == usize::MAX { 0 } else { min }
    }

    fn children(&self) -> [(bool, Option<&TagTree>); 2] {
        [(false, self.zero.as_deref()), (true, self.one.as_deref())]
    }
}

struct EnumDispatch<'a, B: Backend> {
    type_name: &'a Ident,
    enum_name: &'a str,
    format: TagFormat,
    variants: &'a [VariantInfo],
    reader: &'a Ident,
    _marker: std::marker::PhantomData<B>,
}

fn gen_tag_dispatch<B: Backend>(
    node: &TagTree,
    ctx: &EnumDispatch<B>,
    depth: usize,
) -> Result<TokenStream> {
    if node.leaf.is_some() && node.zero.is_none() && node.one.is_none() {
        let idx = node.leaf.unwrap();
        return gen_variant_body::<B>(ctx.reader, ctx.type_name, &ctx.variants[idx]);
    }

    let read_bits = node.min_leaf_depth();
    let int_ty = tag_int_type(read_bits, Span::call_site())?;
    let tag_ident = format_ident!("__tag_{depth}");
    let nbits = Literal::usize_unsuffixed(read_bits);
    let reader = ctx.reader;

    let mut arms: Vec<TokenStream> = Vec::new();
    collect_arms(
        node,
        ctx,
        depth,
        read_bits,
        read_bits,
        &mut Vec::new(),
        &mut arms,
    )?;

    let err_msg = format!("invalid {} tag", ctx.enum_name);
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

fn collect_arms<B: Backend>(
    node: &TagTree,
    ctx: &EnumDispatch<B>,
    depth: usize,
    remaining: usize,
    total_bits: usize,
    prefix: &mut Vec<bool>,
    arms: &mut Vec<TokenStream>,
) -> Result<()> {
    if remaining == 0 {
        let val = prefix.iter().fold(0u64, |a, &b| (a << 1) | b as u64);
        let val_lit = format_match_literal(val, total_bits, ctx.format);
        let body = if node.leaf.is_some() && node.zero.is_none() && node.one.is_none() {
            gen_variant_body::<B>(ctx.reader, ctx.type_name, &ctx.variants[node.leaf.unwrap()])?
        } else {
            gen_tag_dispatch::<B>(node, ctx, depth + 1)?
        };
        arms.push(quote! { #val_lit => #body, });
        return Ok(());
    }

    for (bit, child) in node.children() {
        let Some(child) = child else { continue };
        prefix.push(bit);
        collect_arms::<B>(child, ctx, depth, remaining - 1, total_bits, prefix, arms)?;
        prefix.pop();
    }
    Ok(())
}

fn format_match_literal(val: u64, bit_len: usize, format: TagFormat) -> Literal {
    let s = match format {
        TagFormat::Binary => format!("0b{val:0>bit_len$b}", bit_len = bit_len),
        TagFormat::Hex => format!("0x{val:0>hex_digits$x}", hex_digits = bit_len.div_ceil(4)),
    };
    s.parse()
        .expect("formatted match literal must be a valid Rust literal")
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

fn expand_struct<B: Backend>(input: &DeriveInput, data: &DataStruct) -> Result<TokenStream> {
    let name = &input.ident;
    let attrs = parse_container_attrs(&input.attrs)?;
    B::validate_container_ensure_empty(
        attrs.ensure_empty,
        attrs.ensure_empty_span.unwrap_or_else(|| name.span()),
    )?;

    let reader = B::reader_ident();
    let (constructor, entries) = match &data.fields {
        Fields::Named(f) => {
            let entries = build_entries::<B>(named_field_inputs(&f.named))?;
            let names: Vec<Ident> = entries.iter().map(|e| e.binding.clone()).collect();
            let constructor = quote! { Self { #(#names,)* } };
            (constructor, entries)
        }
        Fields::Unnamed(f) => {
            let entries = build_entries::<B>(tuple_field_inputs(&f.unnamed))?;
            let idents: Vec<Ident> = entries.iter().map(|e| e.binding.clone()).collect();
            let constructor = quote! { Self(#(#idents,)*) };
            (constructor, entries)
        }
        Fields::Unit => {
            return Err(syn::Error::new_spanned(
                name,
                "derive does not support unit structs",
            ));
        }
    };

    let tag_stmt = match &attrs.tag {
        Some(tag) => gen_tag_check(&reader, tag, &name.to_string())?,
        None => quote! {},
    };
    let sections = group_sections(entries)?;
    let section_stmts = sections.iter().map(|s| gen_section(&reader, s));
    let ensure_empty = if attrs.ensure_empty {
        quote! { #reader.ensure_empty()?; }
    } else {
        quote! {}
    };

    let body = quote! {
        use toner::tlb::bits::de::BitReaderExt;
        use toner::tlb::Context;

        #tag_stmt
        #(#section_stmts)*
        #ensure_empty
        Ok(#constructor)
    };

    Ok(B::impl_block(name, &input.generics, body))
}

fn expand_enum<B: Backend>(input: &DeriveInput, data: &DataEnum) -> Result<TokenStream> {
    let name = &input.ident;
    let attrs = parse_container_attrs(&input.attrs)?;
    B::validate_container_ensure_empty(
        attrs.ensure_empty,
        attrs.ensure_empty_span.unwrap_or_else(|| name.span()),
    )?;

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

    let mut root = TagTree::new();
    for (i, v) in variants.iter().enumerate() {
        root.insert(v.tag.bits(), i);
    }

    let format = variants[0].tag.format();
    let reader = B::reader_ident();
    let ctx = EnumDispatch::<B> {
        type_name: name,
        enum_name: &name.to_string(),
        format,
        variants: &variants,
        reader: &reader,
        _marker: std::marker::PhantomData,
    };
    let dispatch = gen_tag_dispatch::<B>(&root, &ctx, 0)?;
    let ensure_empty = if attrs.ensure_empty {
        quote! { #reader.ensure_empty()?; }
    } else {
        quote! {}
    };

    let body = quote! {
        use toner::tlb::bits::de::BitReaderExt;
        use toner::tlb::Context;

        let __result = #dispatch;
        #ensure_empty
        Ok(__result)
    };

    Ok(B::impl_block(name, &input.generics, body))
}

pub fn extend_generics_with_de(
    generics: &syn::Generics,
) -> (TokenStream, TokenStream, TokenStream) {
    let mut extended = generics.clone();
    extended.params.insert(
        0,
        syn::GenericParam::Lifetime(syn::LifetimeParam::new(syn::Lifetime::new(
            "'de",
            Span::call_site(),
        ))),
    );
    let (impl_g, _, _) = extended.split_for_impl();
    let (_, ty_g, where_g) = generics.split_for_impl();
    (quote! { #impl_g }, quote! { #ty_g }, quote! { #where_g })
}
