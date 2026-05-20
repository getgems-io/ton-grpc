use toner_tlb_macros::CellDeserialize;

#[derive(CellDeserialize)]
struct OrphanStart {
    #[tlb(separate_cell_start, bits)]
    a: u8,
    #[tlb(bits)]
    b: u8,
}

fn main() {}
