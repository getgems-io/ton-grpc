use toner_tlb_macros::CellDeserialize;

#[derive(CellDeserialize)]
struct OrphanStart {
    #[tlb(separate_cell_start, unpack)]
    a: u8,
    #[tlb(unpack)]
    b: u8,
}

fn main() {}
