use num_bigint::BigUint;
use toner::tlb::bits::VarInt;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// storage_used$_ cells:(VarUInteger 7) bits:(VarUInteger 7) = StorageUsed;
/// ```
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StorageUsed {
    cells: BigUint,
    bits: BigUint,
}

impl<'de> BitUnpack<'de> for StorageUsed {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let cells = reader.unpack_as::<_, VarInt<3>>(())?;
        let bits = reader.unpack_as::<_, VarInt<3>>(())?;

        Ok(Self { cells, bits })
    }
}
