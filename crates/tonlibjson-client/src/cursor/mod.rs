use crate::block::TonBlockIdExt;

pub mod client;
pub mod discover;
pub mod registry;
pub mod shard_bounds;

pub type Seqno = i32;
pub type ChainId = i32;
pub type ShardId = (i32, i64);

impl From<&TonBlockIdExt> for ShardId {
    fn from(value: &TonBlockIdExt) -> Self {
        (value.workchain, value.shard)
    }
}
