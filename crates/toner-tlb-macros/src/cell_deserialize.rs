use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Fields, Ident, LitStr, Result, Type, punctuated::Punctuated, token::Comma,
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
            let s = format!("0b{:0>width$b}u8", val, width = bit_len);
            s.parse::<proc_macro2::Literal>().unwrap()
        }
        TagFormat::Hex => {
            let hex_digits = bit_len.div_ceil(4);
            let s = format!("0x{:0>width$x}u32", val, width = hex_digits);
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

fn parse_field_mode(attrs: &[syn::Attribute]) -> Result<Option<FieldMode>> {
    let mut kind = None;
    let mut args = None;
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
            } else {
                return Err(meta.error("unknown tlb field attribute"));
            }
            Ok(())
        })?;
    }
    Ok(kind.map(|kind| FieldMode { kind, args }))
}

fn gen_field_parse(field_name: &Ident, mode: &FieldMode, context: &str) -> TokenStream {
    let args = mode
        .args
        .as_ref()
        .map(|expr| quote! { #expr })
        .unwrap_or_else(|| quote! { () });

    match &mode.kind {
        FieldModeKind::Parse => quote! {
            let #field_name = parser.parse(#args)
                .context(#context)?;
        },
        FieldModeKind::ParseAs(ty) => quote! {
            let #field_name = parser.parse_as::<_, #ty>(#args)
                .context(#context)?;
        },
        FieldModeKind::Unpack => quote! {
            let #field_name = parser.unpack(#args)
                .context(#context)?;
        },
        FieldModeKind::UnpackAs(ty) => quote! {
            let #field_name = parser.unpack_as::<_, #ty>(#args)
                .context(#context)?;
        },
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

    let mut field_stmts = Vec::new();
    let mut field_names = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let mode = parse_field_mode(&field.attrs)?.unwrap_or(FieldMode {
            kind: FieldModeKind::Parse,
            args: None,
        });
        let context = field_name.to_string();
        field_stmts.push(gen_field_parse(field_name, &mode, &context));
        field_names.push(field_name);
    }

    let ensure_empty = if container_attrs.ensure_empty {
        quote! { parser.ensure_empty()?; }
    } else {
        quote! {}
    };

    let (_impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl<'de> toner::tlb::de::CellDeserialize<'de> for #name #ty_generics #where_clause {
            type Args = ();

            fn parse(
                parser: &mut toner::tlb::de::CellParser<'de>,
                _args: Self::Args,
            ) -> ::core::result::Result<Self, toner::tlb::de::CellParserError<'de>> {
                use toner::tlb::bits::de::BitReaderExt;
                use toner::tlb::Context;

                #tag_stmt

                #(#field_stmts)*

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

    let mut field_stmts = Vec::new();
    let mut field_idents = Vec::new();

    for (i, field) in fields.iter().enumerate() {
        let field_ident = format_ident!("__field_{}", i);
        let mode = parse_field_mode(&field.attrs)?.unwrap_or(FieldMode {
            kind: FieldModeKind::Parse,
            args: None,
        });
        let context = i.to_string();
        field_stmts.push(gen_field_parse(&field_ident, &mode, &context));
        field_idents.push(field_ident);
    }

    let ensure_empty = if container_attrs.ensure_empty {
        quote! { parser.ensure_empty()?; }
    } else {
        quote! {}
    };

    let (_impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl<'de> toner::tlb::de::CellDeserialize<'de> for #name #ty_generics #where_clause {
            type Args = ();

            fn parse(
                parser: &mut toner::tlb::de::CellParser<'de>,
                _args: Self::Args,
            ) -> ::core::result::Result<Self, toner::tlb::de::CellParserError<'de>> {
                use toner::tlb::bits::de::BitReaderExt;
                use toner::tlb::Context;

                #tag_stmt

                #(#field_stmts)*

                #ensure_empty

                Ok(Self(#(#field_idents,)*))
            }
        }
    })
}

fn gen_tag_validation(attrs: &ContainerAttrs, type_name: &str) -> Result<TokenStream> {
    let Some(tag) = &attrs.tag else {
        return Ok(quote! {});
    };

    let bit_len = tag.bit_len();
    let tag_val = tag.as_u64();
    let err_msg = format!("invalid {type_name} tag");
    let tag_lit = make_literal(tag_val, bit_len, tag.format);

    if bit_len <= 8 {
        let nbits_val = proc_macro2::Literal::usize_unsuffixed(bit_len);
        Ok(quote! {
            let __tag: u8 = parser.unpack_as::<_, toner::tlb::bits::NBits<#nbits_val>>(())?;
            if __tag != #tag_lit as u8 {
                return Err(toner::tlb::Error::custom(format!(
                    concat!(#err_msg, ": 0x{:x}"), __tag
                )));
            }
        })
    } else if bit_len <= 32 {
        Ok(quote! {
            let __tag: u32 = parser.unpack(())?;
            if __tag != #tag_lit as u32 {
                return Err(toner::tlb::Error::custom(format!(
                    concat!(#err_msg, ": 0x{:08x}"), __tag
                )));
            }
        })
    } else {
        Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "tags longer than 32 bits are not supported",
        ))
    }
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
    let mut field_stmts = Vec::new();
    let mut field_names = Vec::new();

    for field in &variant.fields {
        let field_name = field.ident.as_ref().unwrap();
        let mode = parse_field_mode(&field.attrs)?.unwrap_or(FieldMode {
            kind: FieldModeKind::Parse,
            args: None,
        });
        let context = field_name.to_string();
        field_stmts.push(gen_field_parse(field_name, &mode, &context));
        field_names.push(field_name);
    }

    if field_names.is_empty() {
        Ok(quote! { #type_name::#variant_ident })
    } else {
        Ok(quote! {
            {
                #(#field_stmts)*
                #type_name::#variant_ident {
                    #(#field_names,)*
                }
            }
        })
    }
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

fn gen_match_tree(
    node: &TagTreeNode,
    variants: &[VariantInfo],
    type_name: &Ident,
    enum_name: &str,
    depth: usize,
    format: TagFormat,
) -> Result<TokenStream> {
    if let Some(idx) = node.leaf {
        if node.children.is_empty() {
            return gen_variant_body(type_name, &variants[idx]);
        }
    }

    let read_bits = node.min_leaf_depth();

    let tag_ident = format_ident!("__tag_{}", depth);
    let nbits_val = proc_macro2::Literal::usize_unsuffixed(read_bits);

    let mut match_arms: Vec<TokenStream> = Vec::new();
    collect_arms_at_depth(
        node,
        variants,
        type_name,
        enum_name,
        depth,
        read_bits,
        read_bits,
        format,
        &mut Vec::new(),
        &mut match_arms,
    )?;

    let err_msg = format!("invalid {enum_name} tag");

    Ok(quote! {
        {
            let #tag_ident: u8 = parser.unpack_as::<_, toner::tlb::bits::NBits<#nbits_val>>(())?;
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
    variants: &[VariantInfo],
    type_name: &Ident,
    enum_name: &str,
    depth: usize,
    remaining: usize,
    total_bits: usize,
    format: TagFormat,
    prefix: &mut Vec<bool>,
    arms: &mut Vec<TokenStream>,
) -> Result<()> {
    if remaining == 0 {
        let val = prefix.iter().fold(0u8, |acc, &b| (acc << 1) | (b as u8));
        let val_lit = make_literal(val as u64, total_bits, format);

        if let Some(idx) = node.leaf {
            if node.children.is_empty() {
                let body = gen_variant_body(type_name, &variants[idx])?;
                arms.push(quote! { #val_lit => #body, });
                return Ok(());
            }
        }

        let body = gen_match_tree(node, variants, type_name, enum_name, depth + 1, format)?;
        arms.push(quote! { #val_lit => #body, });
        return Ok(());
    }

    for (&bit, child) in &node.children {
        prefix.push(bit);
        collect_arms_at_depth(
            child,
            variants,
            type_name,
            enum_name,
            depth,
            remaining - 1,
            total_bits,
            format,
            prefix,
            arms,
        )?;
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
    let body = gen_match_tree(&root, &variants, name, &name.to_string(), 0, format)?;

    let (_impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl<'de> toner::tlb::de::CellDeserialize<'de> for #name #ty_generics #where_clause {
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
