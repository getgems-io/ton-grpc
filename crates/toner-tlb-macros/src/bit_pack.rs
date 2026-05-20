use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{DeriveInput, Generics, Ident, Result};

use crate::common::{Backend, FieldLayer, SeparateCellMarker, split_generics};
use crate::writer;

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    writer::expand::<BitPackBackend>(input)
}

struct BitPackBackend;

impl Backend for BitPackBackend {
    fn ident() -> Ident {
        format_ident!("writer")
    }

    fn impl_block(name: &Ident, generics: &Generics, body: TokenStream) -> TokenStream {
        let (impl_g, ty_g, where_g) = split_generics(generics);
        quote! {
            impl #impl_g toner::tlb::bits::ser::BitPack for #name #ty_g #where_g {
                type Args = ();

                fn pack<__W>(
                    &self,
                    writer: &mut __W,
                    _args: Self::Args,
                ) -> ::core::result::Result<(), __W::Error>
                where
                    __W: toner::tlb::bits::ser::BitWriter + ?Sized,
                {
                    #body
                }
            }
        }
    }

    fn validate_field_layer(layer: FieldLayer, span: Span) -> Result<()> {
        match layer {
            FieldLayer::Cell => Err(syn::Error::new(
                span,
                "`cell` cannot be used with derive(BitPack); BitPack operates on bits only — use `bits`",
            )),
            FieldLayer::Bits => Ok(()),
        }
    }

    fn validate_separate_cell_marker(marker: SeparateCellMarker, span: Span) -> Result<()> {
        match marker {
            SeparateCellMarker::None => Ok(()),
            _ => Err(syn::Error::new(
                span,
                "`separate_cell_start`/`separate_cell_end` cannot be used with derive(BitPack); BitPack operates on bits only and has no concept of cell references",
            )),
        }
    }

    fn default_layer() -> FieldLayer {
        FieldLayer::Bits
    }
}
