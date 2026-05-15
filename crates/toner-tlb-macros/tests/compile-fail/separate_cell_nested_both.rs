use toner_tlb_macros::CellDeserialize;

#[derive(CellDeserialize)]
struct NestedBoth {
    #[tlb(separate_cell_start, unpack)]
    a: u8,
    #[tlb(separate_cell_start, separate_cell_end, unpack)]
    b: u8,
    #[tlb(separate_cell_end, unpack)]
    c: u8,
}

fn main() {}
