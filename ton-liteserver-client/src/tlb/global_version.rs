use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::tlb::bits::r#as::NBits;

/// ```tlb
/// capabilities#c4 version:uint32 capabilities:uint64 = GlobalVersion;
/// ```
#[derive(Debug, Clone)]
pub struct GlobalVersion {
    pub version: u32,
    pub capabilities: u64,
}

impl BitUnpack for GlobalVersion {
    fn unpack<R>(mut reader: R) -> Result<Self, R::Error>
    where
        R: BitReader,
    {
        let tag: u8 = reader.unpack_as::<_, NBits<8>>()?;
        if tag != 0xC4 {
            unreachable!()
        }

        let version = reader.unpack()?;
        let capabilities = reader.unpack()?;

        Ok(Self {
            version,
            capabilities,
        })
    }
}
