use toner_tlb_macros::CellDeserialize;

#[derive(CellDeserialize)]
struct OrphanEnd {
    #[tlb(bits)]
    a: u8,
    #[tlb(separate_cell_end, bits)]
    b: u8,
}

fn main() {}
