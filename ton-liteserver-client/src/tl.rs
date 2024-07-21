#![allow(dead_code)]
#![allow(unused_mut)]

use adnl_tcp::deserializer::{Deserialize, DeserializeBoxed, Deserializer, DeserializerBoxedError};
use adnl_tcp::serializer::{Serialize, SerializeBoxed, Serializer};
pub use adnl_tcp::types::*;
use std::fmt::{Debug, Display, Formatter};
use ton_client_util::router::route::{BlockCriteria, Route, ToRoute};
use ton_client_util::service::timeout::ToTimeout;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

impl Display for LiteServerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error code: {}, message: {:?}", self.code, self.message)
    }
}

impl std::error::Error for LiteServerError {}

impl TonNodeBlockId {
    pub fn new(workchain: Int, shard: Long, seqno: Int) -> Self {
        Self {
            workchain,
            shard,
            seqno,
        }
    }
}

impl LiteServerLookupBlock {
    pub fn seqno(block_id: TonNodeBlockId) -> Self {
        Self {
            mode: 1,
            id: block_id,
            lt: None,
            utime: None,
        }
    }
}

impl LiteServerGetBlockHeader {
    pub fn new(id: TonNodeBlockIdExt) -> Self {
        Self { id, mode: 0 }
    }
}

impl LiteServerGetBlock {
    pub fn new(id: TonNodeBlockIdExt) -> Self {
        Self { id }
    }
}

impl LiteServerGetAllShardsInfo {
    pub fn new(block_id: TonNodeBlockIdExt) -> Self {
        Self { id: block_id }
    }
}

/// ```tl
/// liteServer.getMasterchainInfo = liteServer.MasterchainInfo;
/// ```
impl ToRoute for LiteServerGetMasterchainInfo {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToTimeout for LiteServerGetMasterchainInfo {}

/// ```tl
/// liteServer.getMasterchainInfoExt mode:# = liteServer.MasterchainInfoExt;
/// ```
impl ToRoute for LiteServerGetMasterchainInfoExt {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToTimeout for LiteServerGetMasterchainInfoExt {}

/// ```tl
/// liteServer.getBlock id:tonNode.blockIdExt = liteServer.BlockData;
/// ```
impl ToRoute for LiteServerGetBlock {
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

impl ToTimeout for LiteServerGetBlock {}

/// ```tl
/// liteServer.getState id:tonNode.blockIdExt = liteServer.BlockState;
/// ```
impl ToRoute for LiteServerGetState {
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

impl ToTimeout for LiteServerGetState {}

/// ```tl
/// liteServer.getBlockHeader id:tonNode.blockIdExt mode:# = liteServer.BlockHeader;
/// ```
impl ToRoute for LiteServerGetBlockHeader {
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

impl ToTimeout for LiteServerGetBlockHeader {}

/// ```tl
/// liteServer.sendMessage body:bytes = liteServer.SendMsgStatus;
/// ```
impl ToRoute for LiteServerSendMessage {
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToTimeout for LiteServerSendMessage {}

/// ```tl
/// liteServer.getAccountState id:tonNode.blockIdExt account:liteServer.accountId = liteServer.AccountState;
/// ```
impl ToRoute for LiteServerGetAccountState {
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

impl ToTimeout for LiteServerGetAccountState {}

/// ```tl
/// liteServer.getAccountStatePrunned id:tonNode.blockIdExt account:liteServer.accountId = liteServer.AccountState;
/// ```
impl ToRoute for LiteServerGetAccountStatePrunned {
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

impl ToTimeout for LiteServerGetAccountStatePrunned {}

/// ```tl
/// liteServer.runSmcMethod mode:# id:tonNode.blockIdExt account:liteServer.accountId method_id:long params:bytes = liteServer.RunMethodResult;
/// ```
impl ToRoute for LiteServerRunSmcMethod {
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

impl ToTimeout for LiteServerRunSmcMethod {}

/// ```tl
/// liteServer.getShardInfo id:tonNode.blockIdExt workchain:int shard:long exact:Bool = liteServer.ShardInfo;
/// ```
impl ToRoute for LiteServerGetShardInfo {
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

impl ToTimeout for LiteServerGetShardInfo {}

/// ```tl
/// liteServer.getAllShardsInfo id:tonNode.blockIdExt = liteServer.AllShardsInfo;
/// ```
impl ToRoute for LiteServerGetAllShardsInfo {
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

impl ToTimeout for LiteServerGetAllShardsInfo {}

/// ```tl
/// liteServer.getOneTransaction id:tonNode.blockIdExt account:liteServer.accountId lt:long = liteServer.TransactionInfo;
/// ```
impl ToRoute for LiteServerGetOneTransaction {
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

impl ToTimeout for LiteServerGetOneTransaction {}

/// ```tl
/// liteServer.getTransactions count:# account:liteServer.accountId lt:long hash:int256 = liteServer.TransactionList;
/// ```
impl ToRoute for LiteServerGetTransactions {
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.account.workchain,
            criteria: BlockCriteria::LogicalTime {
                address: self.account.id,
                lt: self.lt,
            },
        }
    }
}

impl ToTimeout for LiteServerGetTransactions {}

/// ```tl
/// liteServer.lookupBlock mode:# id:tonNode.blockId lt:mode.1?long utime:mode.2?int = liteServer.BlockHeader;
/// ```
impl ToRoute for LiteServerLookupBlock {
    fn to_route(&self) -> Route {
        let criteria = match self.lt.as_ref() {
            None => BlockCriteria::Seqno {
                shard: self.id.shard,
                seqno: self.id.seqno,
            },
            Some(_) => {
                let mut address = [0_u8; 32];
                address[0..8].copy_from_slice(&self.id.shard.to_be_bytes());

                BlockCriteria::LogicalTime {
                    address,
                    lt: self.lt.expect("lt must be defined"),
                }
            }
        };

        Route::Block {
            chain: self.id.workchain,
            criteria,
        }
    }
}

impl ToTimeout for LiteServerLookupBlock {}

/// ```tl
/// liteServer.lookupBlockWithProof mode:# id:tonNode.blockId mc_block_id:tonNode.blockIdExt lt:mode.1?long utime:mode.2?int = liteServer.LookupBlockResult;
/// ```
impl ToRoute for LiteServerLookupBlockWithProof {
    fn to_route(&self) -> Route {
        let criteria = match self.lt.as_ref() {
            None => BlockCriteria::Seqno {
                shard: self.id.shard,
                seqno: self.id.seqno,
            },
            Some(_) => {
                let mut address = [0_u8; 32];
                address[0..8].copy_from_slice(&self.id.shard.to_be_bytes());

                BlockCriteria::LogicalTime {
                    address,
                    lt: self.lt.expect("lt must be defined"),
                }
            }
        };

        Route::Block {
            chain: self.id.workchain,
            criteria,
        }
    }
}

impl ToTimeout for LiteServerLookupBlockWithProof {}

/// ```tl
/// liteServer.listBlockTransactions id:tonNode.blockIdExt mode:# count:# after:mode.7?liteServer.transactionId3 reverse_order:mode.6?true want_proof:mode.5?true = liteServer.BlockTransactions;
/// ```
impl ToRoute for LiteServerListBlockTransactions {
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

impl ToTimeout for LiteServerListBlockTransactions {}

/// ```tl
/// liteServer.listBlockTransactionsExt id:tonNode.blockIdExt mode:# count:# after:mode.7?liteServer.transactionId3 reverse_order:mode.6?true want_proof:mode.5?true = liteServer.BlockTransactionsExt;
/// ```
impl ToRoute for LiteServerListBlockTransactionsExt {
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

impl ToTimeout for LiteServerListBlockTransactionsExt {}

/// ```tl
/// liteServer.getBlockProof mode:# known_block:tonNode.blockIdExt target_block:mode.0?tonNode.blockIdExt = liteServer.PartialBlockProof;
/// ```
impl ToRoute for LiteServerGetBlockProof {
    // TODO[akostylev0] maybe we should use target block if it's defined
    fn to_route(&self) -> Route {
        Route::Block {
            chain: self.known_block.workchain,
            criteria: BlockCriteria::Seqno {
                shard: self.known_block.shard,
                seqno: self.known_block.seqno,
            },
        }
    }
}

impl ToTimeout for LiteServerGetBlockProof {}

/// ```tl
/// liteServer.getConfigAll mode:# id:tonNode.blockIdExt = liteServer.ConfigInfo;
/// ```
impl ToRoute for LiteServerGetConfigAll {
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

impl ToTimeout for LiteServerGetConfigAll {}

/// ```tl
/// liteServer.getConfigParams mode:# id:tonNode.blockIdExt param_list:(vector int) = liteServer.ConfigInfo;
/// ```
impl ToRoute for LiteServerGetConfigParams {
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

impl ToTimeout for LiteServerGetConfigParams {}

/// ```tl
/// liteServer.getValidatorStats#091a58bc mode:# id:tonNode.blockIdExt limit:int start_after:mode.0?int256 modified_after:mode.2?int = liteServer.ValidatorStats;
/// ```
impl ToRoute for LiteServerGetValidatorStats {
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

impl ToTimeout for LiteServerGetValidatorStats {}

/// ```tl
/// liteServer.getLibraries library_list:(vector int256) = liteServer.LibraryResult;
/// ```
impl ToRoute for LiteServerGetLibraries {
    // TODO[akostylev0] I'm not sure if this is correct to implement ToRoute for this kind of request
    fn to_route(&self) -> Route {
        Route::Latest
    }
}

impl ToTimeout for LiteServerGetLibraries {}

/// ```tl
/// liteServer.getLibrariesWithProof id:tonNode.blockIdExt mode:# library_list:(vector int256) = liteServer.LibraryResultWithProof;
/// ```
impl ToRoute for LiteServerGetLibrariesWithProof {
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

impl ToTimeout for LiteServerGetLibrariesWithProof {}

/// ```tl
/// liteServer.getShardBlockProof id:tonNode.blockIdExt = liteServer.ShardBlockProof;
/// ```
impl ToRoute for LiteServerGetShardBlockProof {
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

impl ToTimeout for LiteServerGetShardBlockProof {}

#[cfg(test)]
mod tests {
    use super::*;
    use adnl_tcp::deserializer::from_bytes_boxed;
    use adnl_tcp::serializer::to_bytes_boxed;
    use base64::Engine;

    #[test]
    fn serialize_adnl_query_test() {
        let query = AdnlMessageQuery {
            query_id: hex::decode(
                "77c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c4",
            )
            .unwrap()
            .try_into()
            .unwrap(),
            query: hex::decode("df068c79042ee6b589000000").unwrap(),
        };

        let bytes = to_bytes_boxed(&query);

        assert_eq!(bytes, hex::decode("7af98bb477c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c40cdf068c79042ee6b589000000000000").unwrap())
    }

    #[test]
    fn serialize_liteserver_query_test() {
        let query = LiteServerQuery {
            data: hex::decode("2ee6b589").unwrap(),
        };

        let bytes = to_bytes_boxed(&query);

        assert_eq!(bytes, hex::decode("df068c79042ee6b589000000").unwrap())
    }

    #[test]
    fn serialize_get_masterchain_info_test() {
        let s = LiteServerGetMasterchainInfo::default();

        let bytes = to_bytes_boxed(&s);

        assert_eq!(bytes, hex::decode("2ee6b589").unwrap())
    }

    #[test]
    fn deserialize_adnl_query_test() {
        let bytes = hex::decode("7af98bb477c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c40cdf068c79042ee6b589000000000000").unwrap();

        let query = from_bytes_boxed::<AdnlMessageQuery>(&bytes).unwrap();

        assert_eq!(
            query,
            AdnlMessageQuery {
                query_id: hex::decode(
                    "77c1545b96fa136b8e01cc08338bec47e8a43215492dda6d4d7e286382bb00c4"
                )
                .unwrap()
                .try_into()
                .unwrap(),
                query: hex::decode("df068c79042ee6b589000000").unwrap()
            }
        )
    }

    #[test]
    fn deserialize_masterchain_info_test() {
        let bytes = hex::decode("81288385ffffffff000000000000008027405801e585a47bd5978f6a4fb2b56aa2082ec9deac33aaae19e78241b97522e1fb43d4876851b60521311853f59c002d46b0bd80054af4bce340787a00bd04e01235178b4d3b38b06bb484015faf9821c3ba1c609a25b74f30e1e585b8c8e820ef0976ffffffff17a3a92992aabea785a7a090985a265cd31f323d849da51239737e321fb055695e994fcf4d425c0a6ce6a792594b7173205f740a39cd56f537defd28b48a0f6e").unwrap();

        let masterchain_info = from_bytes_boxed::<LiteServerMasterchainInfo>(&bytes).unwrap();

        eprintln!(
            "{}",
            base64::engine::general_purpose::STANDARD.encode(
                hex::decode("e585a47bd5978f6a4fb2b56aa2082ec9deac33aaae19e78241b97522e1fb43d4")
                    .unwrap()
            )
        );
        eprintln!(
            "{}",
            base64::engine::general_purpose::STANDARD.encode(
                hex::decode("876851b60521311853f59c002d46b0bd80054af4bce340787a00bd04e0123517")
                    .unwrap()
            )
        );

        assert_eq!(
            masterchain_info,
            LiteServerMasterchainInfo {
                last: TonNodeBlockIdExt {
                    workchain: 0xffffffff_u32.to_be() as i32,
                    shard: 0x00000000000080_u64.to_be() as i64,
                    seqno: 0x27405801_u32.to_be() as i32,
                    root_hash: hex::decode(
                        "e585a47bd5978f6a4fb2b56aa2082ec9deac33aaae19e78241b97522e1fb43d4"
                    )
                    .unwrap()
                    .try_into()
                    .unwrap(),
                    file_hash: hex::decode(
                        "876851b60521311853f59c002d46b0bd80054af4bce340787a00bd04e0123517"
                    )
                    .unwrap()
                    .try_into()
                    .unwrap(),
                },
                state_root_hash: hex::decode(
                    "8b4d3b38b06bb484015faf9821c3ba1c609a25b74f30e1e585b8c8e820ef0976"
                )
                .unwrap()
                .try_into()
                .unwrap(),
                init: TonNodeZeroStateIdExt {
                    workchain: 0xffffffff_u32.to_be() as i32,
                    root_hash: hex::decode(
                        "17a3a92992aabea785a7a090985a265cd31f323d849da51239737e321fb05569"
                    )
                    .unwrap()
                    .try_into()
                    .unwrap(),
                    file_hash: hex::decode(
                        "5e994fcf4d425c0a6ce6a792594b7173205f740a39cd56f537defd28b48a0f6e"
                    )
                    .unwrap()
                    .try_into()
                    .unwrap(),
                },
            }
        )
    }
}
