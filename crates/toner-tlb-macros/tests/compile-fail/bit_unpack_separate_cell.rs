use toner_tlb_macros::BitUnpack;

#[derive(BitUnpack)]
struct Bad {
    #[tlb(separate_cell_start, bits)]
    a: u8,
    #[tlb(separate_cell_end, bits)]
    b: u8,
}

fn main() {}
