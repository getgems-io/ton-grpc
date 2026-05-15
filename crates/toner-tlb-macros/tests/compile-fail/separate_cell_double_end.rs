use toner_tlb_macros::CellDeserialize;

#[derive(CellDeserialize)]
struct DoubleEnd {
    #[tlb(separate_cell_start, unpack)]
    a: u8,
    #[tlb(separate_cell_end, unpack)]
    b: u8,
    #[tlb(separate_cell_end, unpack)]
    c: u8,
}

fn main() {}
