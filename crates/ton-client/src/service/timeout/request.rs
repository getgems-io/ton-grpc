use crate::service::timeout::ToTimeout;
use ton_tower::request::*;

impl ToTimeout for GetMasterchainInfo {}
impl ToTimeout for Sync {}
impl ToTimeout for SendMessage {}
impl ToTimeout for SendMessageReturningHash {}
impl ToTimeout for GetAccountState {}
impl ToTimeout for GetAccountStateOnBlock {}
impl ToTimeout for GetAccountStateByTransaction {}
impl ToTimeout for GetAccountTransactions {}
impl ToTimeout for GetShardAccountCell {}
impl ToTimeout for GetShardAccountCellOnBlock {}
impl ToTimeout for GetShardAccountCellByTransaction {}
impl ToTimeout for RunGetMethod {}
impl ToTimeout for LookUpBlockBySeqno {}
impl ToTimeout for LookUpBlockByLt {}
impl ToTimeout for GetShards {}
impl ToTimeout for GetBlockHeader {}
impl ToTimeout for GetTransactionIds {}
impl ToTimeout for GetTransactions {}
