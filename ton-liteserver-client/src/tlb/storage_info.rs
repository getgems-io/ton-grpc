use crate::tlb::storage_extra_info::StorageExtraInfo;
use crate::tlb::storage_used::StorageUsed;
use num_bigint::BigUint;
use toner::tlb::bits::de::{BitReader, BitReaderExt};
use toner::ton::bits::de::BitUnpack;
use toner::ton::currency::Grams;
use url::quirks::username;

/// ```tlb
/// storage_info$_ used:StorageUsed storage_extra:StorageExtraInfo last_paid:uint32
///               due_payment:(Maybe Grams) = StorageInfo;
/// ```
pub struct StorageInfo {
    used: StorageUsed,
    storage_extra: StorageExtraInfo,
    last_paid: u32,
    due_payment: Option<BigUint>,
}

impl<'de> BitUnpack<'de> for StorageInfo {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let used = reader.unpack(())?;
        let storage_extra = reader.unpack(())?;
        let last_paid = reader.unpack(())?;
        let due_payment = reader.unpack_as::<_, Option<Grams>>(())?;

        Ok(StorageInfo {
            used,
            storage_extra,
            last_paid,
            due_payment,
        })
    }
}
