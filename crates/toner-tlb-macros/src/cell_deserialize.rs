use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{DeriveInput, Generics, Ident, Result};

use crate::common::{self, Backend, FieldModeKind, SeparateCellMarker, extend_generics_with_de};

pub fn expand(input: DeriveInput) -> Result<TokenStream> {
    common::expand::<CellBackend>(input)
}

struct CellBackend;

impl Backend for CellBackend {
    fn reader_ident() -> Ident {
        format_ident!("parser")
    }

    fn impl_block(name: &Ident, generics: &Generics, body: TokenStream) -> TokenStream {
        let (impl_g, ty_g, where_g) = extend_generics_with_de(generics);
        quote! {
            impl #impl_g toner::tlb::de::CellDeserialize<'de> for #name #ty_g #where_g {
                type Args = ();

                fn parse(
                    parser: &mut toner::tlb::de::CellParser<'de>,
                    _args: Self::Args,
                ) -> ::core::result::Result<Self, toner::tlb::de::CellParserError<'de>> {
                    #body
                }
            }
        }
    }

    fn validate_field_mode(_kind: &FieldModeKind, _span: Span) -> Result<()> {
        Ok(())
    }

    fn validate_separate_cell_marker(_marker: SeparateCellMarker, _span: Span) -> Result<()> {
        Ok(())
    }

    fn validate_container_ensure_empty(_ensure_empty: bool, _span: Span) -> Result<()> {
        Ok(())
    }

    fn default_mode_kind() -> FieldModeKind {
        FieldModeKind::Parse
    }
}
