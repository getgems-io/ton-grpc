use toner_tlb_macros::BitUnpack;

#[derive(BitUnpack)]
struct Bad {
    #[tlb(separate_cell_start, unpack)]
    a: u8,
    #[tlb(separate_cell_end, unpack)]
    b: u8,
}

fn main() {}
