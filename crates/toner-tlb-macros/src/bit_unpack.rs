use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{DeriveInput, Generics, Ident, Result};

use crate::common::{self, Backend, FieldModeKind, SeparateCellMarker, extend_generics_with_de};

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    common::expand::<BitBackend>(input)
}

struct BitBackend;

impl Backend for BitBackend {
    fn reader_ident() -> Ident {
        format_ident!("reader")
    }

    fn impl_block(name: &Ident, generics: &Generics, body: TokenStream) -> TokenStream {
        let (impl_g, ty_g, where_g) = extend_generics_with_de(generics);
        quote! {
            impl #impl_g toner::tlb::bits::de::BitUnpack<'de> for #name #ty_g #where_g {
                type Args = ();

                fn unpack<__R>(
                    reader: &mut __R,
                    _args: Self::Args,
                ) -> ::core::result::Result<Self, __R::Error>
                where
                    __R: toner::tlb::bits::de::BitReader<'de> + ?Sized,
                {
                    #body
                }
            }
        }
    }

    fn validate_field_mode(kind: &FieldModeKind, span: Span) -> Result<()> {
        match kind {
            FieldModeKind::Parse => Err(syn::Error::new(
                span,
                "`parse` cannot be used with derive(BitUnpack); use `unpack` instead",
            )),
            FieldModeKind::ParseAs(_) => Err(syn::Error::new(
                span,
                "`parse_as` cannot be used with derive(BitUnpack); use `unpack_as` instead",
            )),
            FieldModeKind::Unpack | FieldModeKind::UnpackAs(_) => Ok(()),
        }
    }

    fn validate_separate_cell_marker(marker: SeparateCellMarker, span: Span) -> Result<()> {
        match marker {
            SeparateCellMarker::None => Ok(()),
            _ => Err(syn::Error::new(
                span,
                "`separate_cell_start`/`separate_cell_end` cannot be used with derive(BitUnpack); BitUnpack operates on bits only and has no concept of cell references",
            )),
        }
    }

    fn validate_container_ensure_empty(ensure_empty: bool, span: Span) -> Result<()> {
        if ensure_empty {
            return Err(syn::Error::new(
                span,
                "`ensure_empty` cannot be used with derive(BitUnpack); BitReader has no notion of trailing data",
            ));
        }
        Ok(())
    }

    fn default_mode_kind() -> FieldModeKind {
        FieldModeKind::Unpack
    }
}
