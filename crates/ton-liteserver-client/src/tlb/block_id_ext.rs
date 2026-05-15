use crate::tlb::shard_ident::ShardIdent;
use toner_tlb_macros::BitUnpack;

/// ```tlb
/// block_id_ext$_
/// shard_id:ShardIdent
/// seq_no:uint32
/// root_hash:bits256
/// file_hash:bits256 = BlockIdExt;
/// ```
#[derive(Debug, Clone, BitUnpack)]
pub struct BlockIdExt {
    pub shard_id: ShardIdent,
    pub seq_no: u32,
    pub root_hash: [u8; 32],
    pub file_hash: [u8; 32],
}
