use toner::tlb::bits::NBits;
use toner_tlb_macros::BitUnpack;

/// `tlb
/// shard_ident$00
/// shard_pfx_bits:(#<= 60)
/// workchain_id:int32
/// shard_prefix:uint64 = ShardIdent;
/// ```
#[derive(Debug, Clone, BitUnpack)]
#[tlb(tag = "0b00")]
pub struct ShardIdent {
    #[tlb(unpack_as = "NBits<6>")]
    pub shard_pfx_bits: u8,
    pub workchain_id: i32,
    pub shard_prefix: u64,
}
