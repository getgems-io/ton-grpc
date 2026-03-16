use crate::block::{BlocksHeader, TonBlockIdExt};
use crate::cursor::shard_bounds::ShardBounds;
use crate::cursor::{ChainId, Seqno, ShardId};
use dashmap::{DashMap, DashSet};
use ton_client_util::router::route::BlockCriteria;
use ton_client_util::router::shard_prefix::ShardPrefix;
use ton_client_util::router::BlockAvailability;

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

    pub fn available(&self, chain: &ChainId, criteria: &BlockCriteria) -> BlockAvailability {
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
                        .fold(BlockAvailability::NotPresent, |result, bounds| {
                            if matches!(result, BlockAvailability::Available) {
                                return BlockAvailability::Available;
                            }

                            let availability = bounds.contains_lt(*lt);

                            if matches!(result, BlockAvailability::NotAvailable) {
                                return match availability {
                                    BlockAvailability::Available => BlockAvailability::Available,
                                    _ => BlockAvailability::NotAvailable,
                                };
                            }

                            if matches!(result, BlockAvailability::Unknown) {
                                return match availability {
                                    BlockAvailability::Available => BlockAvailability::Available,
                                    BlockAvailability::NotAvailable => {
                                        BlockAvailability::NotAvailable
                                    }
                                    _ => BlockAvailability::Unknown,
                                };
                            }

                            availability
                        })
                })
                .unwrap_or(BlockAvailability::Unknown),
            BlockCriteria::Seqno { shard, seqno } => {
                let shard_id = (*chain, *shard);
                let Some(bounds) = self.shard_bounds_registry.get(&shard_id) else {
                    return BlockAvailability::Unknown;
                };

                bounds.contains_seqno(*seqno)
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

#[cfg(test)]
mod tests {
    use crate::block::BlocksHeader;
    use crate::cursor::registry::BlockCriteria;
    use crate::cursor::registry::Registry;
    use ton_client_util::router::BlockAvailability;

    #[test]
    pub fn registry_available_lt_exactly_one_available() {
        let registry = given_registry(vec![((1, 95), (1, 105))]);
        let block_criteria = BlockCriteria::LogicalTime {
            address: [0; 32],
            lt: 100,
        };

        let actual = registry.available(&0, &block_criteria);

        assert_eq!(actual, BlockAvailability::Available);
    }

    fn given_registry(shards: Vec<((i64, i64), (i64, i64))>) -> Registry {
        let registry = Registry::default();
        for (shard_left, shard_right) in shards {
            let left_header = given_block_header_with_lt(shard_left.0, shard_left.1);
            let right_header = given_block_header_with_lt(shard_right.0, shard_right.1);
            registry.upsert_left(&left_header);
            registry.upsert_right(&right_header);
        }

        registry
    }

    fn given_block_header_with_lt(shard: i64, lt: i64) -> BlocksHeader {
        BlocksHeader {
            id: crate::block::TonBlockIdExt {
                workchain: 0,
                shard,
                seqno: 100,
                root_hash: "root_hash".to_string(),
                file_hash: "file_hash".to_string(),
            },
            global_id: 0,
            version: 0,
            flags: 0,
            after_merge: false,
            after_split: false,
            before_split: false,
            want_merge: false,
            want_split: false,
            validator_list_hash_short: 0,
            catchain_seqno: 0,
            min_ref_mc_seqno: 0,
            is_key_block: false,
            prev_key_block_seqno: 0,
            start_lt: lt,
            end_lt: lt + 10,
            gen_utime: 0,
            vert_seqno: 0,
            prev_blocks: vec![],
        }
    }
}
