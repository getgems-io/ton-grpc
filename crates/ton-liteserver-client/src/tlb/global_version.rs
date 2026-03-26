use toner::tlb::bits::NBits;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// capabilities#c4 version:uint32 capabilities:uint64 = GlobalVersion;
/// ```
#[derive(Debug, Clone)]
pub struct GlobalVersion {
    pub version: u32,
    pub capabilities: u64,
}

impl<'de> BitUnpack<'de> for GlobalVersion {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let tag: u8 = reader.unpack_as::<_, NBits<8>>(())?;
        if tag != 0xC4 {
            unreachable!()
        }

        let version = reader.unpack(())?;
        let capabilities = reader.unpack(())?;

        Ok(Self {
            version,
            capabilities,
        })
    }
}
