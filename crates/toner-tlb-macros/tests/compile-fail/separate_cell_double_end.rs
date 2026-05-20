use toner_tlb_macros::CellDeserialize;

#[derive(CellDeserialize)]
struct DoubleEnd {
    #[tlb(separate_cell_start, bits)]
    a: u8,
    #[tlb(separate_cell_end, bits)]
    b: u8,
    #[tlb(separate_cell_end, bits)]
    c: u8,
}

fn main() {}
