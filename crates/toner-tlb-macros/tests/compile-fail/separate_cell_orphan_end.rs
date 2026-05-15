use toner_tlb_macros::CellDeserialize;

#[derive(CellDeserialize)]
struct OrphanEnd {
    #[tlb(unpack)]
    a: u8,
    #[tlb(separate_cell_end, unpack)]
    b: u8,
}

fn main() {}
