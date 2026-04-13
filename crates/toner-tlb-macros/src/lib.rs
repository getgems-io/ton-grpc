use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod cell_deserialize;

/// Derive macro for `CellDeserialize<'de>` trait.
///
/// # Struct attributes
///
/// - `#[tlb(tag = "0b0111")]` or `#[tlb(tag = "0x11ef55aa")]` — validate a fixed tag prefix
/// - `#[tlb(ensure_empty)]` — call `parser.ensure_empty()` after parsing all fields
///
/// # Field attributes
///
/// - `#[tlb(parse)]` — use `parser.parse(())?` (default for `CellDeserialize` types)
/// - `#[tlb(parse_as = "Ref<ParseFully>")]` — use `parser.parse_as::<_, Ref<ParseFully>>(())?`
/// - `#[tlb(unpack)]` — use `parser.unpack(())?` (for `BitUnpack` types)
/// - `#[tlb(unpack_as = "Grams")]` — use `parser.unpack_as::<_, Grams>(())?`
/// - `#[tlb(parse_as = "Hashmap<Ref<T>, C>", args = "(64, (), ())")]` — pass custom args instead of `()`
///
/// # Enum variant attributes
///
/// - `#[tlb(tag = "0b000")]` — tag value for this variant
/// - Tree-like tags are resolved automatically from the tag values.
///   E.g. `0b000`, `0b00100`, `0b00101` — the macro builds an optimal prefix tree
///   and reads the minimum number of bits at each level.
#[proc_macro_derive(CellDeserialize, attributes(tlb))]
pub fn derive_cell_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    cell_deserialize::expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
