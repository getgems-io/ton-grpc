use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Result, punctuated::Punctuated, token::Comma};

use crate::common::{
    ContainerAttrs, DefaultFieldMode, FieldEntry, FieldMode, FieldModeKind, FieldSection,
    SeparateCellMarker, TagFormat, TagTreeNode, VariantInfo, collect_named_field_inputs,
    collect_tuple_field_inputs, gen_field_entries, group_fields_into_sections, int_type_for_bits,
    make_literal, parse_container_attrs, parse_variant_info,
};

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    match &input.data {
        Data::Struct(data) => expand_struct(&input, data),
        Data::Enum(data) => expand_enum(&input, data),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "BitUnpack cannot be derived for unions",
        )),
    }
}

fn validate_for_bit_unpack(entries: &[FieldEntry]) -> Result<()> {
    for entry in entries {
        match entry.separate_cell {
            SeparateCellMarker::None => {}
            _ => {
                return Err(syn::Error::new(
                    entry.span,
                    "`separate_cell_start`/`separate_cell_end` cannot be used with derive(BitUnpack); BitUnpack operates on bits only and has no concept of cell references",
                ));
            }
        }
        match &entry.mode.kind {
            FieldModeKind::Parse => {
                return Err(syn::Error::new(
                    entry.span,
                    "`parse` cannot be used with derive(BitUnpack); use `unpack` instead",
                ));
            }
            FieldModeKind::ParseAs(_) => {
                return Err(syn::Error::new(
                    entry.span,
                    "`parse_as` cannot be used with derive(BitUnpack); use `unpack_as` instead",
                ));
            }
            FieldModeKind::Unpack | FieldModeKind::UnpackAs(_) => {}
        }
    }
    Ok(())
}

fn validate_container_for_bit_unpack(attrs: &ContainerAttrs, ident: &Ident) -> Result<()> {
    if attrs.ensure_empty {
        return Err(syn::Error::new_spanned(
            ident,
            "`ensure_empty` cannot be used with derive(BitUnpack); BitReader has no notion of trailing data",
        ));
    }
    Ok(())
}

fn gen_field_unpack(field_name: &Ident, mode: &FieldMode, context: &str) -> TokenStream {
    let args = mode
        .args
        .as_ref()
        .map(|expr| quote! { #expr })
        .unwrap_or_else(|| quote! { () });

    match &mode.kind {
        FieldModeKind::Unpack => quote! {
            let #field_name = reader.unpack(#args)
                .context(#context)?;
        },
        FieldModeKind::UnpackAs(ty) => quote! {
            let #field_name = reader.unpack_as::<_, #ty>(#args)
                .context(#context)?;
        },
        FieldModeKind::Parse | FieldModeKind::ParseAs(_) => unreachable!(
            "validate_for_bit_unpack should have rejected parse/parse_as before reaching code generation",
        ),
    }
}

fn gen_section(section: &FieldSection) -> TokenStream {
    match section {
        FieldSection::Flat(entries) => {
            let stmts = entries
                .iter()
                .map(|e| gen_field_unpack(&e.binding, &e.mode, &e.context));
            quote! { #(#stmts)* }
        }
        FieldSection::SeparateCell(_) => unreachable!(
            "validate_for_bit_unpack should have rejected separate_cell markers before reaching code generation",
        ),
    }
}

fn expand_struct(input: &DeriveInput, data: &syn::DataStruct) -> Result<TokenStream> {
    let name = &input.ident;
    let container_attrs = parse_container_attrs(&input.attrs)?;
    validate_container_for_bit_unpack(&container_attrs, name)?;

    match &data.fields {
        Fields::Named(f) => expand_named_struct(input, &container_attrs, &f.named),
        Fields::Unnamed(f) => expand_tuple_struct(input, &container_attrs, &f.unnamed),
        Fields::Unit => Err(syn::Error::new_spanned(
            name,
            "BitUnpack derive does not support unit structs",
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

    let entries = gen_field_entries(collect_named_field_inputs(fields), DefaultFieldMode::Unpack)?;
    validate_for_bit_unpack(&entries)?;
    let field_names: Vec<Ident> = entries.iter().map(|e| e.binding.clone()).collect();
    let sections = group_fields_into_sections(entries)?;
    let section_stmts = sections.iter().map(gen_section);

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl<'de, #impl_generics> toner::tlb::bits::de::BitUnpack<'de> for #name #ty_generics #where_clause {
            type Args = ();

            fn unpack<__R>(
                reader: &mut __R,
                _args: Self::Args,
            ) -> ::core::result::Result<Self, __R::Error>
            where
                __R: toner::tlb::bits::de::BitReader<'de> + ?Sized,
            {
                use toner::tlb::bits::de::BitReaderExt;
                use toner::tlb::Context;

                #tag_stmt

                #(#section_stmts)*

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

    let entries = gen_field_entries(collect_tuple_field_inputs(fields), DefaultFieldMode::Unpack)?;
    validate_for_bit_unpack(&entries)?;
    let field_idents: Vec<Ident> = entries.iter().map(|e| e.binding.clone()).collect();
    let sections = group_fields_into_sections(entries)?;
    let section_stmts = sections.iter().map(gen_section);

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl<'de, #impl_generics> toner::tlb::bits::de::BitUnpack<'de> for #name #ty_generics #where_clause {
            type Args = ();

            fn unpack<__R>(
                reader: &mut __R,
                _args: Self::Args,
            ) -> ::core::result::Result<Self, __R::Error>
            where
                __R: toner::tlb::bits::de::BitReader<'de> + ?Sized,
            {
                use toner::tlb::bits::de::BitReaderExt;
                use toner::tlb::Context;

                #tag_stmt

                #(#section_stmts)*

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

    let int_ty = int_type_for_bits(bit_len)?;
    let nbits_val = proc_macro2::Literal::usize_unsuffixed(bit_len);
    let hex_width = bit_len.div_ceil(4);
    let fmt_str = format!("{err_msg}: 0x{{:0>{hex_width}x}}");

    Ok(quote! {
        let __tag: #int_ty = reader.unpack_as::<_, toner::tlb::bits::NBits<#nbits_val>>(())?;
        if __tag != #tag_lit as #int_ty {
            return Err(toner::tlb::Error::custom(format!(
                #fmt_str, __tag
            )));
        }
    })
}

fn gen_variant_body(type_name: &Ident, variant: &VariantInfo) -> Result<TokenStream> {
    let variant_ident = &variant.ident;

    if variant.fields.is_empty() {
        return Ok(quote! { #type_name::#variant_ident });
    }

    let entries = gen_field_entries(
        collect_named_field_inputs(&variant.fields),
        DefaultFieldMode::Unpack,
    )?;
    validate_for_bit_unpack(&entries)?;
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
            let #tag_ident: #int_ty = reader.unpack_as::<_, toner::tlb::bits::NBits<#nbits_val>>(())?;
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
    let container_attrs = parse_container_attrs(&input.attrs)?;
    validate_container_for_bit_unpack(&container_attrs, name)?;

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
        impl<'de, #impl_generics> toner::tlb::bits::de::BitUnpack<'de> for #name #ty_generics #where_clause {
            type Args = ();

            fn unpack<__R>(
                reader: &mut __R,
                _args: Self::Args,
            ) -> ::core::result::Result<Self, __R::Error>
            where
                __R: toner::tlb::bits::de::BitReader<'de> + ?Sized,
            {
                use toner::tlb::bits::de::BitReaderExt;
                use toner::tlb::Context;

                Ok(#body)
            }
        }
    })
}
