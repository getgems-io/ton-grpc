use darling::FromField;
use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Data, DataEnum, DataStruct, DeriveInput, Field, Fields, Ident, Index, Result,
    punctuated::Punctuated, spanned::Spanned, token::Comma,
};

use crate::common::{
    Backend, FieldLayer, FieldMode, RawField, SeparateCellMarker, TagValue, VariantInfo,
    parse_container_attrs, parse_variant, tag_int_type,
};

struct FieldEntry {
    access: TokenStream,
    binding: Option<Ident>,
    mode: FieldMode,
    context: String,
    separate_cell: SeparateCellMarker,
    span: Span,
}

#[derive(Clone, Copy)]
enum AccessKind {
    SelfDirect,
    Binding,
}

fn build_entries<B: Backend>(
    fields: &Punctuated<Field, Comma>,
    named: bool,
    access: AccessKind,
) -> Result<Vec<FieldEntry>> {
    fields
        .iter()
        .enumerate()
        .map(|(i, f)| build_entry::<B>(f, i, named, access))
        .collect()
}

fn build_entry<B: Backend>(
    f: &Field,
    index: usize,
    named: bool,
    access: AccessKind,
) -> Result<FieldEntry> {
    let raw = RawField::from_field(f)?;
    let span = f.span();

    let (access_tokens, binding, context) = match access {
        AccessKind::SelfDirect => {
            if named {
                let id = raw.ident.clone().expect("named field must have ident");
                let context = id.to_string();
                (quote! { self.#id }, None, context)
            } else {
                let idx = Index::from(index);
                (quote! { self.#idx }, None, index.to_string())
            }
        }
        AccessKind::Binding => {
            if named {
                let id = raw.ident.clone().expect("named field must have ident");
                let context = id.to_string();
                (quote! { #id }, Some(id), context)
            } else {
                let id = format_ident!("__field_{index}");
                (quote! { #id }, Some(id), index.to_string())
            }
        }
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
        access: access_tokens,
        binding,
        mode,
        context,
        separate_cell: marker,
        span,
    })
}

/// Linear codegen for a sequence of fields where `^[ ... ]` (TLB "separate cell")
/// blocks are inlined: each block opens a fresh child cell builder, fields inside
/// are written into it, and the block is attached as a reference at its end.
fn gen_field_stmts(writer: &Ident, entries: &[FieldEntry]) -> Result<TokenStream> {
    let sub = format_ident!("__separate_cell_builder");
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
                let mut #sub = toner::tlb::Cell::builder();
            });
            block_open_span = Some(span);
        }

        let active = if block_open_span.is_some() {
            &sub
        } else {
            writer
        };
        out.extend(gen_field_call(active, entry));

        if closes {
            out.extend(quote! {
                #writer
                    .store_as::<_, toner::tlb::Ref>(&#sub.into_cell(), ())
                    .context("^]")?;
            });
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

fn gen_field_call(writer: &Ident, entry: &FieldEntry) -> TokenStream {
    let FieldEntry {
        access,
        mode,
        context,
        ..
    } = entry;
    let args = match &mode.args {
        Some(expr) => quote! { #expr },
        None => quote! { () },
    };
    let call = match (mode.layer, &mode.as_ty) {
        (FieldLayer::Cell, None) => quote! { #writer.store(&#access, #args) },
        (FieldLayer::Cell, Some(ty)) => quote! { #writer.store_as::<_, &#ty>(&#access, #args) },
        (FieldLayer::Bits, None) => quote! { #writer.pack(&#access, #args) },
        (FieldLayer::Bits, Some(ty)) => quote! { #writer.pack_as::<_, &#ty>(&#access, #args) },
    };
    quote! { #call.context(#context)?; }
}

pub fn gen_tag_store(writer: &Ident, tag: &TagValue, context: &str) -> Result<TokenStream> {
    let bit_len = tag.bit_len();
    let int_ty = tag_int_type(bit_len, tag.span())?;
    let nbits = Literal::usize_unsuffixed(bit_len);
    let tag_lit = tag.literal();
    Ok(quote! {
        #writer
            .pack_as::<#int_ty, toner::tlb::bits::NBits<#nbits>>(
                #tag_lit as #int_ty,
                (),
            )
            .context(#context)?;
    })
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
    let body = quote! {
        use toner::tlb::bits::ser::BitWriterExt;
        use toner::tlb::Context;
        #inner
        Ok(())
    };
    B::impl_block(&input.ident, &input.generics, body)
}

fn expand_struct<B: Backend>(input: &DeriveInput, data: &DataStruct) -> Result<TokenStream> {
    let attrs = parse_container_attrs(input)?;
    let writer = B::ident();

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
    let entries = build_entries::<B>(fields, named, AccessKind::SelfDirect)?;
    let stmts = gen_field_stmts(&writer, &entries)?;
    let tag_stmt = attrs
        .tag
        .as_ref()
        .map(|tag| gen_tag_store(&writer, tag, "tag"))
        .transpose()?;

    Ok(wrap_impl::<B>(
        input,
        quote! {
            #tag_stmt
            #stmts
        },
    ))
}

fn expand_enum<B: Backend>(input: &DeriveInput, data: &DataEnum) -> Result<TokenStream> {
    parse_container_attrs(input)?;
    let writer = B::ident();
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

    let arms: Vec<TokenStream> = variants
        .iter()
        .map(|v| gen_variant_arm::<B>(&writer, name, v))
        .collect::<Result<_>>()?;

    let inner = quote! {
        match self {
            #(#arms)*
        }
    };
    Ok(wrap_impl::<B>(input, inner))
}

fn gen_variant_arm<B: Backend>(
    writer: &Ident,
    type_name: &Ident,
    variant: &VariantInfo,
) -> Result<TokenStream> {
    let variant_ident = &variant.ident;
    let tag_store = gen_tag_store(writer, &variant.tag, "tag")?;

    if variant.fields.is_empty() {
        return Ok(quote! {
            #type_name::#variant_ident => {
                #tag_store
            }
        });
    }

    let named = variant
        .fields
        .first()
        .map(|f| f.ident.is_some())
        .unwrap_or(true);
    let entries = build_entries::<B>(&variant.fields, named, AccessKind::Binding)?;
    let bindings: Vec<&Ident> = entries
        .iter()
        .map(|e| e.binding.as_ref().expect("variant fields are always bound"))
        .collect();
    let stmts = gen_field_stmts(writer, &entries)?;

    let pattern = if named {
        quote! { #type_name::#variant_ident { #(#bindings,)* } }
    } else {
        quote! { #type_name::#variant_ident(#(#bindings,)*) }
    };

    Ok(quote! {
        #pattern => {
            #tag_store
            #stmts
        }
    })
}
