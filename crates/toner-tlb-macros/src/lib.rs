use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod bit_pack;
mod bit_unpack;
mod cell_deserialize;
mod cell_serialize;
mod common;
mod reader;
mod writer;

#[proc_macro_derive(CellDeserialize, attributes(tlb))]
pub fn derive_cell_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    cell_deserialize::expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_derive(BitUnpack, attributes(tlb))]
pub fn derive_bit_unpack(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    bit_unpack::expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_derive(CellSerialize, attributes(tlb))]
pub fn derive_cell_serialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    cell_serialize::expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_derive(BitPack, attributes(tlb))]
pub fn derive_bit_pack(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    bit_pack::expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
