use toner_tlb_macros::BitUnpack;

#[derive(BitUnpack)]
struct Bad {
    #[tlb(cell)]
    val: bool,
}

fn main() {}
