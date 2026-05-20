use toner_tlb_macros::BitUnpack;

#[derive(BitUnpack)]
struct Bad {
    #[tlb(cell, as = "u8")]
    val: u8,
}

fn main() {}
