use toner_tlb_macros::BitUnpack;

#[derive(BitUnpack)]
struct Bad {
    #[tlb(parse)]
    val: bool,
}

fn main() {}
