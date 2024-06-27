use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// ext_blk_ref$_
/// end_lt:uint64
/// seq_no:uint32
/// root_hash:bits256
/// file_hash:bits256
///   = ExtBlkRef;
/// ```
#[derive(Debug, Clone)]
pub struct ExtBlkRef {
    pub end_lt: u64,
    pub seq_no: u32,
    pub root_hash: [u8; 32],
    pub file_hash: [u8; 32],
}

impl BitUnpack for ExtBlkRef {
    fn unpack<R>(mut reader: R) -> Result<Self, R::Error>
    where
        R: BitReader,
    {
        let end_lt = reader.unpack()?;
        let seq_no = reader.unpack()?;
        let root_hash = reader.unpack()?;
        let file_hash = reader.unpack()?;

        Ok(Self {
            end_lt,
            seq_no,
            root_hash,
            file_hash,
        })
    }
}
