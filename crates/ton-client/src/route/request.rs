use crate::route::{BlockCriteria, Route, ToRoute};
use ton_tower::request::*;

impl ToRoute for GetAccountStateOnBlock {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.block_id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.block_id.shard,
                seqno: self.block_id.seqno,
            },
        }
    }
}

impl ToRoute for GetShardAccountCellOnBlock {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.block_id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.block_id.shard,
                seqno: self.block_id.seqno,
            },
        }
    }
}

impl ToRoute for GetMasterchainInfo {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToRoute for Sync {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToRoute for SendMessage {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToRoute for SendMessageReturningHash {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToRoute for GetAccountState {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToRoute for GetAccountStateByTransaction {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.address.workchain_id(),
            criteria: BlockCriteria::LogicalTime {
                address: self.address.data_as_bytes(),
                lt: self.transaction_id.lt,
            },
        }
    }
}

impl ToRoute for GetAccountTransactions {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToRoute for GetShardAccountCell {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToRoute for GetShardAccountCellByTransaction {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.address.workchain_id(),
            criteria: BlockCriteria::LogicalTime {
                address: self.address.data_as_bytes(),
                lt: self.transaction_id.lt,
            },
        }
    }
}

impl ToRoute for RunGetMethod {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToRoute for LookUpBlockBySeqno {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.chain,
            criteria: BlockCriteria::Seqno {
                shard: self.shard,
                seqno: self.seqno,
            },
        }
    }
}

impl ToRoute for LookUpBlockByLt {
    fn to_route(&self) -> Route {
        let mut address = [0u8; 32];
        address[0..8].copy_from_slice(&self.shard.to_be_bytes());

        Route::Block {
            chain: self.chain,
            criteria: BlockCriteria::LogicalTime {
                address,
                lt: self.lt,
            },
        }
    }
}

impl ToRoute for GetShards {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.block_id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.block_id.shard,
                seqno: self.block_id.seqno,
            },
        }
    }
}

impl ToRoute for GetBlockHeader {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.id.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.id.shard,
                seqno: self.id.seqno,
            },
        }
    }
}

impl ToRoute for GetTransactionIds {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.block.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.block.shard,
                seqno: self.block.seqno,
            },
        }
    }
}

impl ToRoute for GetTransactions {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.block.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.block.shard,
                seqno: self.block.seqno,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use ton_address::SmartContractAddress;
    use ton_tower::response::BlockIdExt;

    #[rstest]
    #[case::masterchain_info(GetMasterchainInfo::default().to_route(), Route::Latest)]
    #[case::look_up_block_by_lt(
        LookUpBlockByLt { chain: 0, shard: 0, lt: 123456 }.to_route(),
        Route::Block {
            chain: 0,
            criteria: BlockCriteria::LogicalTime {
                address: [0; 32],
                lt: 123456,
            },
        }
    )]
    #[case::look_up_block_by_seqno(
        LookUpBlockBySeqno { chain: -1, shard: i64::MIN, seqno: 42 }.to_route(),
        block_route(-1, i64::MIN, 42)
    )]
    #[case::get_shards(
        GetShards { block_id: block_id(0, 1, 10) }.to_route(),
        block_route(0, 1, 10)
    )]
    #[case::get_account_state_on_block(
        GetAccountStateOnBlock { address: addr(), block_id: block_id(-1, i64::MIN, 42) }.to_route(),
        block_route(-1, i64::MIN, 42)
    )]
    #[case::get_shard_account_cell_on_block(
        GetShardAccountCellOnBlock { address: addr(), block_id: block_id(0, 1, 10) }.to_route(),
        block_route(0, 1, 10)
    )]
    fn to_route(#[case] actual: Route, #[case] expected: Route) {
        assert_eq!(actual, expected);
    }

    fn addr() -> SmartContractAddress {
        SmartContractAddress::raw(0, [0; 32])
    }

    fn block_id(workchain: i32, shard: i64, seqno: i32) -> BlockIdExt {
        BlockIdExt {
            workchain,
            shard,
            seqno,
            root_hash: String::new(),
            file_hash: String::new(),
        }
    }

    fn block_route(chain: i32, shard: i64, seqno: i32) -> Route {
        Route::Block {
            chain,
            criteria: BlockCriteria::Seqno { shard, seqno },
        }
    }
}
