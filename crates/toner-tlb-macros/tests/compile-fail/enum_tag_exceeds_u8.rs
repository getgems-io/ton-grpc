use toner_tlb_macros::CellDeserialize;

#[derive(CellDeserialize)]
enum Wide {
    #[tlb(tag = "0b000000000000000000000000000000000")]
    A,
    #[tlb(tag = "0b111111111111111111111111111111111")]
    B,
}

fn main() {}
