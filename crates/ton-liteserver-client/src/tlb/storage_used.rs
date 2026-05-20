use num_bigint::BigUint;
use toner::tlb::bits::VarInt;
use toner_tlb_macros::BitUnpack;

/// ```tlb
/// storage_used$_ cells:(VarUInteger 7) bits:(VarUInteger 7) = StorageUsed;
/// ```
#[derive(Debug, Clone, Eq, PartialEq, BitUnpack)]
pub struct StorageUsed {
    #[tlb(bits, as = "VarInt<3>")]
    cells: BigUint,
    #[tlb(bits, as = "VarInt<3>")]
    bits: BigUint,
}
