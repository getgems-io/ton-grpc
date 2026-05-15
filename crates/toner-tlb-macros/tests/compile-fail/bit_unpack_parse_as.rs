use toner_tlb_macros::BitUnpack;

#[derive(BitUnpack)]
struct Bad {
    #[tlb(parse_as = "u8")]
    val: u8,
}

fn main() {}
