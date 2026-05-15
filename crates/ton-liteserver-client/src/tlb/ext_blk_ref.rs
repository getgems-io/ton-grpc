use toner_tlb_macros::BitUnpack;

/// ```tlb
/// ext_blk_ref$_
/// end_lt:uint64
/// seq_no:uint32
/// root_hash:bits256
/// file_hash:bits256
///   = ExtBlkRef;
/// ```
#[derive(Debug, Clone, BitUnpack)]
pub struct ExtBlkRef {
    pub end_lt: u64,
    pub seq_no: u32,
    pub root_hash: [u8; 32],
    pub file_hash: [u8; 32],
}
