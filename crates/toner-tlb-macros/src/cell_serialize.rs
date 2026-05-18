use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{DeriveInput, Generics, Ident, Result};

use crate::common::{Backend, FieldLayer, SeparateCellMarker, split_generics};
use crate::writer;

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    writer::expand::<CellSerializeBackend>(input)
}

struct CellSerializeBackend;

impl Backend for CellSerializeBackend {
    fn ident() -> Ident {
        format_ident!("builder")
    }

    fn impl_block(name: &Ident, generics: &Generics, body: TokenStream) -> TokenStream {
        let (impl_g, ty_g, where_g) = split_generics(generics);
        quote! {
            impl #impl_g toner::tlb::ser::CellSerialize for #name #ty_g #where_g {
                type Args = ();

                fn store(
                    &self,
                    builder: &mut toner::tlb::ser::CellBuilder,
                    _args: Self::Args,
                ) -> ::core::result::Result<(), toner::tlb::ser::CellBuilderError> {
                    #body
                }
            }
        }
    }

    fn validate_field_layer(_layer: FieldLayer, _span: Span) -> Result<()> {
        Ok(())
    }

    fn validate_separate_cell_marker(_marker: SeparateCellMarker, _span: Span) -> Result<()> {
        Ok(())
    }

    fn default_layer() -> FieldLayer {
        FieldLayer::Cell
    }
}
