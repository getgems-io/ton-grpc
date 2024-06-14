use crate::tlb::ext_blk_ref::ExtBlkRef;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// master_info$_ master:ExtBlkRef = BlkMasterInfo;
/// ```
#[derive(Debug, Clone)]
pub struct BlkMasterInfo {
    pub master: ExtBlkRef,
}

impl BitUnpack for BlkMasterInfo {
    fn unpack<R>(mut reader: R) -> Result<Self, R::Error>
    where
        R: BitReader,
    {
        let master = reader.unpack()?;

        Ok(Self { master })
    }
}
