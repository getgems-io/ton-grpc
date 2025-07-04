use crate::block::{BlocksHeader, TonBlockIdExt};
use crate::cursor::shard_bounds::ShardBounds;
use crate::cursor::{ChainId, Seqno, ShardId};
use dashmap::{DashMap, DashSet};
use ton_client_util::router::route::BlockCriteria;
use ton_client_util::router::shard_prefix::ShardPrefix;

type ShardRegistry = DashMap<ChainId, DashSet<ShardId>>;
type ShardBoundsRegistry = DashMap<ShardId, ShardBounds>;

#[derive(Default)]
pub struct Registry {
    shard_registry: ShardRegistry,
    shard_bounds_registry: ShardBoundsRegistry,
}

impl Registry {
    pub fn right_next(&self, shard_id: ShardId) -> Option<Seqno> {
        self.shard_bounds_registry
            .get(&shard_id)
            .and_then(|s| s.right_next_seqno())
    }

    pub fn get_last_seqno(&self, shard_id: &ShardId) -> Option<Seqno> {
        self.shard_bounds_registry
            .get(shard_id)
            .and_then(|s| s.right().map(|h| h.id.seqno))
    }

    pub fn upsert_left(&self, header: &BlocksHeader) {
        let shard_id = (header.id.workchain, header.id.shard);

        self.update_shard_registry(&shard_id);

        tracing::trace!(
            chaid_id = header.id.workchain,
            shard_id = header.id.shard,
            seqno = header.id.seqno,
            "left block"
        );

        self.shard_bounds_registry
            .entry(shard_id)
            .and_modify(|b| {
                b.left_replace(header.clone());
            })
            .or_insert_with(|| ShardBounds::from_left(header.clone()));
    }

    pub fn upsert_right(&self, header: &BlocksHeader) {
        let shard_id = (header.id.workchain, header.id.shard);

        self.update_shard_registry(&shard_id);

        tracing::trace!(
            chaid_id = header.id.workchain,
            shard_id = header.id.shard,
            seqno = header.id.seqno,
            "right block"
        );

        self.shard_bounds_registry
            .entry(shard_id)
            .and_modify(|b| {
                b.right_replace(header.clone());
            })
            .or_insert_with(|| ShardBounds::from_right(header.clone()));
    }

    pub fn upsert_right_end(&self, block_id: &TonBlockIdExt) {
        let shard_id = (block_id.workchain, block_id.shard);

        self.update_shard_registry(&shard_id);

        tracing::trace!(
            chaid_id = block_id.workchain,
            shard_id = block_id.shard,
            seqno = block_id.seqno,
            "right end block"
        );

        self.shard_bounds_registry
            .entry(shard_id)
            .and_modify(|b| {
                b.right_seqno_replace(block_id.seqno);
            })
            .or_insert_with(|| ShardBounds::from_right_seqno(block_id.seqno));
    }

    pub fn update_shard_registry(&self, shard_id: &ShardId) {
        let entry = self.shard_registry.entry(shard_id.0).or_default();

        if entry.contains(shard_id) {
            return;
        }

        tracing::trace!(chaid_id = shard_id.0, shard_id = shard_id.1, "new shard");

        entry.insert(*shard_id);
    }

    pub fn contains(&self, chain: &ChainId, criteria: &BlockCriteria, not_available: bool) -> bool {
        match criteria {
            BlockCriteria::LogicalTime { address, lt } => self
                .shard_registry
                .get(chain)
                .map(|shard_ids| {
                    shard_ids
                        .iter()
                        .filter_map(|shard_id| {
                            ShardPrefix::from_shard_id(shard_id.1 as u64)
                                .matches(address)
                                .then(|| self.shard_bounds_registry.get(&shard_id))
                                .flatten()
                        })
                        .any(|bounds| bounds.contains_lt(*lt, not_available))
                })
                .unwrap_or(false),
            BlockCriteria::Seqno { shard, seqno } => {
                let shard_id = (*chain, *shard);
                let Some(bounds) = self.shard_bounds_registry.get(&shard_id) else {
                    return false;
                };

                bounds.contains_seqno(*seqno, not_available)
            }
        }
    }

    pub fn edges_defined(&self, shard_id: &ShardId) -> bool {
        let Some(shard_bounds) = self.shard_bounds_registry.get(shard_id) else {
            return false;
        };

        shard_bounds.left().is_some()
    }
}
