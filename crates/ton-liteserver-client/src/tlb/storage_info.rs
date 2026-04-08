use crate::tlb::storage_extra_info::StorageExtraInfo;
use crate::tlb::storage_used::StorageUsed;
use num_bigint::BigUint;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::ton::currency::Grams;

/// ```tlb
/// storage_info$_ used:StorageUsed storage_extra:StorageExtraInfo last_paid:uint32
///               due_payment:(Maybe Grams) = StorageInfo;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
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
        Ok(Self {
            used: reader.unpack(())?,
            storage_extra: reader.unpack(())?,
            last_paid: reader.unpack(())?,
            due_payment: reader.unpack_as::<_, Option<Grams>>(())?,
        })
    }
}
