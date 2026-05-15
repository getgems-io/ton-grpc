use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Fields, Ident, LitStr, Result, Type, punctuated::Punctuated,
    spanned::Spanned, token::Comma,
};

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    match &input.data {
        Data::Struct(data) => expand_struct(&input, data),
        Data::Enum(data) => expand_enum(&input, data),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "CellDeserialize cannot be derived for unions",
        )),
    }
}

#[derive(Default)]
struct ContainerAttrs {
    tag: Option<TagValue>,
    ensure_empty: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TagFormat {
    Binary,
    Hex,
}

struct TagValue {
    bits: Vec<bool>,
    format: TagFormat,
}

impl TagValue {
    fn parse_from_str(s: &str) -> Result<Self> {
        let (bits, format) = if let Some(rest) = s.strip_prefix("0b") {
            (parse_binary_digits(rest)?, TagFormat::Binary)
        } else if let Some(rest) = s.strip_prefix("0x") {
            (parse_hex_to_bits(rest)?, TagFormat::Hex)
        } else if let Some(rest) = s.strip_prefix('#') {
            (parse_hex_to_bits(rest)?, TagFormat::Hex)
        } else if let Some(rest) = s.strip_prefix('$') {
            (parse_binary_digits(rest)?, TagFormat::Binary)
        } else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "tag must start with 0b, 0x, # or $",
            ));
        };

        Ok(TagValue { bits, format })
    }

    fn bit_len(&self) -> usize {
        self.bits.len()
    }

    fn as_u64(&self) -> u64 {
        self.bits
            .iter()
            .fold(0u64, |acc, &b| (acc << 1) | (b as u64))
    }
}

fn make_literal(val: u64, bit_len: usize, format: TagFormat) -> proc_macro2::Literal {
    match format {
        TagFormat::Binary => {
            let s = format!("0b{:0>width$b}", val, width = bit_len);
            s.parse::<proc_macro2::Literal>().unwrap()
        }
        TagFormat::Hex => {
            let hex_digits = bit_len.div_ceil(4);
            let s = format!("0x{:0>width$x}", val, width = hex_digits);
            s.parse::<proc_macro2::Literal>().unwrap()
        }
    }
}

fn parse_binary_digits(s: &str) -> Result<Vec<bool>> {
    s.chars()
        .map(|c| match c {
            '0' => Ok(false),
            '1' => Ok(true),
            _ => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("invalid binary digit: {c}"),
            )),
        })
        .collect()
}

fn parse_hex_to_bits(s: &str) -> Result<Vec<bool>> {
    let val = u64::from_str_radix(s, 16).map_err(|e| {
        syn::Error::new(proc_macro2::Span::call_site(), format!("invalid hex: {e}"))
    })?;
    let bit_count = s.len() * 4;
    Ok((0..bit_count).rev().map(|i| (val >> i) & 1 == 1).collect())
}

fn parse_container_attrs(attrs: &[syn::Attribute]) -> Result<ContainerAttrs> {
    let mut result = ContainerAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("tlb") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                result.tag = Some(TagValue::parse_from_str(&lit.value())?);
            } else if meta.path.is_ident("ensure_empty") {
                result.ensure_empty = true;
            } else {
                return Err(meta.error("unknown tlb attribute"));
            }
            Ok(())
        })?;
    }
    Ok(result)
}

struct FieldMode {
    kind: FieldModeKind,
    args: Option<syn::Expr>,
}

enum FieldModeKind {
    Parse,
    ParseAs(Type),
    Unpack,
    UnpackAs(Type),
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum SeparateCellMarker {
    #[default]
    None,
    Start,
    End,
    Both,
}

#[derive(Default)]
struct FieldAttrs {
    mode: Option<FieldMode>,
    separate_cell: SeparateCellMarker,
}

fn parse_field_attrs(attrs: &[syn::Attribute]) -> Result<FieldAttrs> {
    let mut kind: Option<FieldModeKind> = None;
    let mut args: Option<syn::Expr> = None;
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
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                let ty: Type = syn::parse_str(&lit.value())?;
                kind = Some(FieldModeKind::ParseAs(ty));
            } else if meta.path.is_ident("unpack") {
                if meta.input.peek(syn::Token![=]) {
                    return Err(meta.error("use `unpack_as` for typed unpack"));
                }
                kind = Some(FieldModeKind::Unpack);
            } else if meta.path.is_ident("unpack_as") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                let ty: Type = syn::parse_str(&lit.value())?;
                kind = Some(FieldModeKind::UnpackAs(ty));
            } else if meta.path.is_ident("args") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                let expr: syn::Expr = syn::parse_str(&lit.value())?;
                args = Some(expr);
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

    let separate_cell = match (start, end) {
        (false, false) => SeparateCellMarker::None,
        (true, false) => SeparateCellMarker::Start,
        (false, true) => SeparateCellMarker::End,
        (true, true) => SeparateCellMarker::Both,
    };

    Ok(FieldAttrs {
        mode: kind.map(|kind| FieldMode { kind, args }),
        separate_cell,
    })
}

fn gen_field_parse(
    parser_ident: &Ident,
    field_name: &Ident,
    mode: &FieldMode,
    context: &str,
) -> TokenStream {
    let args = mode
        .args
        .as_ref()
        .map(|expr| quote! { #expr })
        .unwrap_or_else(|| quote! { () });

    match &mode.kind {
        FieldModeKind::Parse => quote! {
            let #field_name = #parser_ident.parse(#args)
                .context(#context)?;
        },
        FieldModeKind::ParseAs(ty) => quote! {
            let #field_name = #parser_ident.parse_as::<_, #ty>(#args)
                .context(#context)?;
        },
        FieldModeKind::Unpack => quote! {
            let #field_name = #parser_ident.unpack(#args)
                .context(#context)?;
        },
        FieldModeKind::UnpackAs(ty) => quote! {
            let #field_name = #parser_ident.unpack_as::<_, #ty>(#args)
                .context(#context)?;
        },
    }
}

struct FieldEntry {
    binding: Ident,
    mode: FieldMode,
    context: String,
    separate_cell: SeparateCellMarker,
    span: proc_macro2::Span,
}

/// Models a flat field run or a `^[ ... ]` (TLB "separate cell") block,
/// loaded as a fresh child cell and required to be fully consumed.
enum FieldSection {
    Flat(Vec<FieldEntry>),
    SeparateCell(Vec<FieldEntry>),
}

fn group_fields_into_sections(entries: Vec<FieldEntry>) -> Result<Vec<FieldSection>> {
    let mut sections: Vec<FieldSection> = Vec::new();
    let mut current_flat: Vec<FieldEntry> = Vec::new();
    let mut current_separate: Option<Vec<FieldEntry>> = None;

    for entry in entries {
        match (current_separate.as_mut(), entry.separate_cell) {
            (None, SeparateCellMarker::None) => current_flat.push(entry),
            (None, SeparateCellMarker::Start) => {
                if !current_flat.is_empty() {
                    sections.push(FieldSection::Flat(std::mem::take(&mut current_flat)));
                }
                current_separate = Some(vec![entry]);
            }
            (None, SeparateCellMarker::Both) => {
                if !current_flat.is_empty() {
                    sections.push(FieldSection::Flat(std::mem::take(&mut current_flat)));
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
                let mut buf = current_separate.take().unwrap();
                buf.push(entry);
                sections.push(FieldSection::SeparateCell(buf));
            }
        }
    }

    if let Some(open) = current_separate {
        let span = open
            .first()
            .map(|f| f.span)
            .unwrap_or_else(proc_macro2::Span::call_site);
        return Err(syn::Error::new(
            span,
            "`separate_cell_start` without a matching `separate_cell_end`",
        ));
    }

    if !current_flat.is_empty() {
        sections.push(FieldSection::Flat(current_flat));
    }

    Ok(sections)
}

fn gen_field_entries(
    fields: impl IntoIterator<Item = (Ident, String, proc_macro2::Span, Vec<syn::Attribute>)>,
) -> Result<Vec<FieldEntry>> {
    let mut entries = Vec::new();
    for (binding, context, span, attrs) in fields {
        let parsed = parse_field_attrs(&attrs)?;
        let mode = parsed.mode.unwrap_or(FieldMode {
            kind: FieldModeKind::Parse,
            args: None,
        });
        entries.push(FieldEntry {
            binding,
            mode,
            context,
            separate_cell: parsed.separate_cell,
            span,
        });
    }
    Ok(entries)
}

fn gen_section(section: &FieldSection) -> TokenStream {
    match section {
        FieldSection::Flat(entries) => {
            let parser_ident: Ident = format_ident!("parser");
            let stmts = entries
                .iter()
                .map(|e| gen_field_parse(&parser_ident, &e.binding, &e.mode, &e.context));
            quote! { #(#stmts)* }
        }
        FieldSection::SeparateCell(entries) => {
            let sub_parser: Ident = format_ident!("__separate_cell_parser");
            let stmts = entries
                .iter()
                .map(|e| gen_field_parse(&sub_parser, &e.binding, &e.mode, &e.context));
            quote! {
                let mut #sub_parser: toner::tlb::de::CellParser<'de> = parser
                    .parse_as::<toner::tlb::de::CellParser<'de>, toner::tlb::Ref>(())
                    .context("^[")?;
                #(#stmts)*
                #sub_parser.ensure_empty().context("^]")?;
            }
        }
    }
}

fn expand_struct(input: &DeriveInput, data: &syn::DataStruct) -> Result<TokenStream> {
    let name = &input.ident;
    let container_attrs = parse_container_attrs(&input.attrs)?;

    match &data.fields {
        Fields::Named(f) => expand_named_struct(input, &container_attrs, &f.named),
        Fields::Unnamed(f) => expand_tuple_struct(input, &container_attrs, &f.unnamed),
        Fields::Unit => Err(syn::Error::new_spanned(
            name,
            "CellDeserialize derive does not support unit structs",
        )),
    }
}

fn expand_named_struct(
    input: &DeriveInput,
    container_attrs: &ContainerAttrs,
    fields: &Punctuated<syn::Field, Comma>,
) -> Result<TokenStream> {
    let name = &input.ident;
    let tag_stmt = gen_tag_validation(container_attrs, &name.to_string())?;

    let entries = gen_field_entries(fields.iter().map(|f| {
        let ident = f.ident.as_ref().unwrap().clone();
        let context = ident.to_string();
        let span = f.span();
        (ident, context, span, f.attrs.clone())
    }))?;
    let field_names: Vec<Ident> = entries.iter().map(|e| e.binding.clone()).collect();
    let sections = group_fields_into_sections(entries)?;
    let section_stmts = sections.iter().map(gen_section);

    let ensure_empty = if container_attrs.ensure_empty {
        quote! { parser.ensure_empty()?; }
    } else {
        quote! {}
    };

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl<'de, #impl_generics> toner::tlb::de::CellDeserialize<'de> for #name #ty_generics #where_clause {
            type Args = ();

            fn parse(
                parser: &mut toner::tlb::de::CellParser<'de>,
                _args: Self::Args,
            ) -> ::core::result::Result<Self, toner::tlb::de::CellParserError<'de>> {
                use toner::tlb::bits::de::BitReaderExt;
                use toner::tlb::Context;

                #tag_stmt

                #(#section_stmts)*

                #ensure_empty

                Ok(Self {
                    #(#field_names,)*
                })
            }
        }
    })
}

fn expand_tuple_struct(
    input: &DeriveInput,
    container_attrs: &ContainerAttrs,
    fields: &Punctuated<syn::Field, Comma>,
) -> Result<TokenStream> {
    let name = &input.ident;
    let tag_stmt = gen_tag_validation(container_attrs, &name.to_string())?;

    let entries = gen_field_entries(fields.iter().enumerate().map(|(i, f)| {
        let ident = format_ident!("__field_{}", i);
        let context = i.to_string();
        let span = f.span();
        (ident, context, span, f.attrs.clone())
    }))?;
    let field_idents: Vec<Ident> = entries.iter().map(|e| e.binding.clone()).collect();
    let sections = group_fields_into_sections(entries)?;
    let section_stmts = sections.iter().map(gen_section);

    let ensure_empty = if container_attrs.ensure_empty {
        quote! { parser.ensure_empty()?; }
    } else {
        quote! {}
    };

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl<'de, #impl_generics> toner::tlb::de::CellDeserialize<'de> for #name #ty_generics #where_clause {
            type Args = ();

            fn parse(
                parser: &mut toner::tlb::de::CellParser<'de>,
                _args: Self::Args,
            ) -> ::core::result::Result<Self, toner::tlb::de::CellParserError<'de>> {
                use toner::tlb::bits::de::BitReaderExt;
                use toner::tlb::Context;

                #tag_stmt

                #(#section_stmts)*

                #ensure_empty

                Ok(Self(#(#field_idents,)*))
            }
        }
    })
}

fn int_type_for_bits(bit_len: usize) -> Result<TokenStream> {
    match bit_len {
        0..=8 => Ok(quote! { u8 }),
        9..=16 => Ok(quote! { u16 }),
        17..=32 => Ok(quote! { u32 }),
        _ => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("tags longer than 32 bits are not supported (got {bit_len})"),
        )),
    }
}

fn gen_tag_validation(attrs: &ContainerAttrs, type_name: &str) -> Result<TokenStream> {
    let Some(tag) = &attrs.tag else {
        return Ok(quote! {});
    };

    let bit_len = tag.bit_len();
    let tag_val = tag.as_u64();
    let err_msg = format!("invalid {type_name} tag");
    let tag_lit = make_literal(tag_val, bit_len, tag.format);

    let int_ty = int_type_for_bits(bit_len)?;
    let nbits_val = proc_macro2::Literal::usize_unsuffixed(bit_len);
    let hex_width = bit_len.div_ceil(4);
    let fmt_str = format!("{err_msg}: 0x{{:0>{hex_width}x}}");

    Ok(quote! {
        let __tag: #int_ty = parser.unpack_as::<_, toner::tlb::bits::NBits<#nbits_val>>(())?;
        if __tag != #tag_lit as #int_ty {
            return Err(toner::tlb::Error::custom(format!(
                #fmt_str, __tag
            )));
        }
    })
}

struct VariantInfo {
    ident: Ident,
    tag: TagValue,
    fields: Punctuated<syn::Field, Comma>,
}

fn parse_variant_info(variant: &syn::Variant) -> Result<VariantInfo> {
    let mut tag = None;
    for attr in &variant.attrs {
        if !attr.path().is_ident("tlb") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                let value = meta.value()?;
                let lit: LitStr = value.parse()?;
                tag = Some(TagValue::parse_from_str(&lit.value())?);
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

fn gen_variant_body(type_name: &Ident, variant: &VariantInfo) -> Result<TokenStream> {
    let variant_ident = &variant.ident;

    if variant.fields.is_empty() {
        return Ok(quote! { #type_name::#variant_ident });
    }

    let entries = gen_field_entries(variant.fields.iter().map(|f| {
        let ident = f.ident.as_ref().unwrap().clone();
        let context = ident.to_string();
        let span = f.span();
        (ident, context, span, f.attrs.clone())
    }))?;
    let field_names: Vec<Ident> = entries.iter().map(|e| e.binding.clone()).collect();
    let sections = group_fields_into_sections(entries)?;
    let section_stmts = sections.iter().map(gen_section);

    Ok(quote! {
        {
            #(#section_stmts)*
            #type_name::#variant_ident {
                #(#field_names,)*
            }
        }
    })
}

struct TagTreeNode {
    leaf: Option<usize>,
    children: BTreeMap<bool, TagTreeNode>,
}

impl TagTreeNode {
    fn new() -> Self {
        Self {
            leaf: None,
            children: BTreeMap::new(),
        }
    }

    fn insert(&mut self, bits: &[bool], variant_idx: usize) {
        if bits.is_empty() {
            self.leaf = Some(variant_idx);
            return;
        }
        self.children
            .entry(bits[0])
            .or_insert_with(TagTreeNode::new)
            .insert(&bits[1..], variant_idx);
    }

    fn min_leaf_depth(&self) -> usize {
        if self.leaf.is_some() {
            return 0;
        }
        1 + self
            .children
            .values()
            .map(|c| c.min_leaf_depth())
            .min()
            .unwrap_or(0)
    }
}

struct MatchTreeCtx<'a> {
    variants: &'a [VariantInfo],
    type_name: &'a Ident,
    enum_name: &'a str,
    format: TagFormat,
}

fn gen_match_tree(node: &TagTreeNode, ctx: &MatchTreeCtx, depth: usize) -> Result<TokenStream> {
    if let Some(idx) = node.leaf {
        if node.children.is_empty() {
            return gen_variant_body(ctx.type_name, &ctx.variants[idx]);
        }
    }

    let read_bits = node.min_leaf_depth();
    let int_ty = int_type_for_bits(read_bits)?;

    let tag_ident = format_ident!("__tag_{}", depth);
    let nbits_val = proc_macro2::Literal::usize_unsuffixed(read_bits);

    let mut match_arms: Vec<TokenStream> = Vec::new();
    collect_arms_at_depth(
        node,
        ctx,
        depth,
        read_bits,
        read_bits,
        &mut Vec::new(),
        &mut match_arms,
    )?;

    let err_msg = format!("invalid {} tag", ctx.enum_name);

    Ok(quote! {
        {
            let #tag_ident: #int_ty = parser.unpack_as::<_, toner::tlb::bits::NBits<#nbits_val>>(())?;
            match #tag_ident {
                #(#match_arms)*
                _ => return Err(toner::tlb::Error::custom(
                    format!(concat!(#err_msg, ": {}"), #tag_ident)
                )),
            }
        }
    })
}

fn collect_arms_at_depth(
    node: &TagTreeNode,
    ctx: &MatchTreeCtx,
    depth: usize,
    remaining: usize,
    total_bits: usize,
    prefix: &mut Vec<bool>,
    arms: &mut Vec<TokenStream>,
) -> Result<()> {
    if remaining == 0 {
        let val = prefix.iter().fold(0u64, |acc, &b| (acc << 1) | (b as u64));
        let val_lit = make_literal(val, total_bits, ctx.format);

        if let Some(idx) = node.leaf {
            if node.children.is_empty() {
                let body = gen_variant_body(ctx.type_name, &ctx.variants[idx])?;
                arms.push(quote! { #val_lit => #body, });
                return Ok(());
            }
        }

        let body = gen_match_tree(node, ctx, depth + 1)?;
        arms.push(quote! { #val_lit => #body, });
        return Ok(());
    }

    for (&bit, child) in &node.children {
        prefix.push(bit);
        collect_arms_at_depth(child, ctx, depth, remaining - 1, total_bits, prefix, arms)?;
        prefix.pop();
    }
    Ok(())
}

fn expand_enum(input: &DeriveInput, data: &syn::DataEnum) -> Result<TokenStream> {
    let name = &input.ident;

    let mut variants: Vec<VariantInfo> = Vec::new();
    for variant in &data.variants {
        variants.push(parse_variant_info(variant)?);
    }

    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            name,
            "enum must have at least one variant",
        ));
    }

    let mut root = TagTreeNode::new();
    for (i, v) in variants.iter().enumerate() {
        root.insert(&v.tag.bits, i);
    }

    let format = variants
        .first()
        .map(|v| v.tag.format)
        .unwrap_or(TagFormat::Binary);
    let ctx = MatchTreeCtx {
        variants: &variants,
        type_name: name,
        enum_name: &name.to_string(),
        format,
    };
    let body = gen_match_tree(&root, &ctx, 0)?;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl<'de, #impl_generics> toner::tlb::de::CellDeserialize<'de> for #name #ty_generics #where_clause {
            type Args = ();

            fn parse(
                parser: &mut toner::tlb::de::CellParser<'de>,
                _args: Self::Args,
            ) -> ::core::result::Result<Self, toner::tlb::de::CellParserError<'de>> {
                use toner::tlb::bits::de::BitReaderExt;
                use toner::tlb::Context;

                Ok(#body)
            }
        }
    })
}
