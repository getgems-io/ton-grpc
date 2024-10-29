use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::tlb::bits::r#as::NBits;

/// `tlb
/// shard_ident$00
/// shard_pfx_bits:(#<= 60)
/// workchain_id:int32
/// shard_prefix:uint64 = ShardIdent;
/// ```
#[derive(Debug, Clone)]
pub struct ShardIdent {
    pub shard_pfx_bits: u8,
    pub workchain_id: i32,
    pub shard_prefix: u64,
}

impl BitUnpack for ShardIdent {
    fn unpack<R>(mut reader: R) -> Result<Self, R::Error>
    where
        R: BitReader,
    {
        let tag: u8 = reader.unpack_as::<_, NBits<2>>()?;
        if tag != 0x00 {
            unreachable!()
        }

        let shard_pfx_bits = reader.unpack_as::<_, NBits<6>>()?;
        let workchain_id = reader.unpack()?;
        let shard_prefix = reader.unpack()?;

        Ok(Self {
            shard_pfx_bits,
            workchain_id,
            shard_prefix,
        })
    }
}
