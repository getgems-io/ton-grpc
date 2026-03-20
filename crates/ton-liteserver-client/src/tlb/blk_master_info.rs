use crate::tlb::ext_blk_ref::ExtBlkRef;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// master_info$_ master:ExtBlkRef = BlkMasterInfo;
/// ```
#[derive(Debug, Clone)]
pub struct BlkMasterInfo {
    pub master: ExtBlkRef,
}

impl<'de> BitUnpack<'de> for BlkMasterInfo {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let master = reader.unpack(())?;

        Ok(Self { master })
    }
}
