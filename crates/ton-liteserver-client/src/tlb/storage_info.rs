use crate::tlb::storage_extra_info::StorageExtraInfo;
use crate::tlb::storage_used::StorageUsed;
use num_bigint::BigUint;
use toner::ton::currency::Grams;
use toner_tlb_macros::BitUnpack;

/// ```tlb
/// storage_info$_ used:StorageUsed storage_extra:StorageExtraInfo last_paid:uint32
///               due_payment:(Maybe Grams) = StorageInfo;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, BitUnpack)]
pub struct StorageInfo {
    used: StorageUsed,
    storage_extra: StorageExtraInfo,
    last_paid: u32,
    #[tlb(unpack_as = "Option<Grams>")]
    due_payment: Option<BigUint>,
}
