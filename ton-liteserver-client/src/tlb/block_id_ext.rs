use crate::tlb::shard_ident::ShardIdent;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};

/// ```tlb
/// block_id_ext$_
/// shard_id:ShardIdent
/// seq_no:uint32
/// root_hash:bits256
/// file_hash:bits256 = BlockIdExt;
/// ```
#[derive(Debug, Clone)]
pub struct BlockIdExt {
    pub shard_id: ShardIdent,
    pub seq_no: u32,
    pub root_hash: [u8; 32],
    pub file_hash: [u8; 32],
}

impl BitUnpack for BlockIdExt {
    fn unpack<R>(mut reader: R) -> Result<Self, R::Error>
    where
        R: BitReader,
    {
        let shard_id = reader.unpack()?;
        let seq_no = reader.unpack()?;
        let root_hash = reader.unpack()?;
        let file_hash = reader.unpack()?;

        Ok(Self {
            shard_id,
            seq_no,
            root_hash,
            file_hash,
        })
    }
}
