use toner_tlb_macros::BitUnpack;

#[derive(BitUnpack)]
#[tlb(ensure_empty)]
struct Bad {
    #[tlb(unpack)]
    val: bool,
}

fn main() {}
