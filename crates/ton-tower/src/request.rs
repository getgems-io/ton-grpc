use crate::Request;
use crate::response::*;
use ton_address::SmartContractAddress;

#[derive(Debug, Clone, Default)]
pub struct GetMasterchainInfo {}

impl Request for GetMasterchainInfo {
    type Response = MasterchainInfo;
}

#[derive(Debug, Clone)]
pub struct LookUpBlockBySeqno {
    pub chain: i32,
    pub shard: i64,
    pub seqno: i32,
}

impl Request for LookUpBlockBySeqno {
    type Response = BlockIdExt;
}

#[derive(Debug, Clone)]
pub struct LookUpBlockByLt {
    pub chain: i32,
    pub shard: i64,
    pub lt: i64,
}

impl Request for LookUpBlockByLt {
    type Response = BlockIdExt;
}

#[derive(Debug, Clone)]
pub struct GetShards {
    pub block_id: BlockIdExt,
}

impl Request for GetShards {
    type Response = Vec<BlockIdExt>;
}

#[derive(Debug, Clone)]
pub struct GetBlockHeader {
    pub id: BlockIdExt,
}

impl Request for GetBlockHeader {
    type Response = BlockHeader;
}

#[derive(Debug, Clone)]
pub struct GetTransactionIds {
    pub block: BlockIdExt,
    pub after: Option<ShortTxId>,
    pub reverse: bool,
    pub count: i32,
}

impl Request for GetTransactionIds {
    type Response = BlockTransactions;
}

#[derive(Debug, Clone)]
pub struct GetTransactions {
    pub block: BlockIdExt,
    pub after: Option<ShortTxId>,
    pub reverse: bool,
    pub count: i32,
}

impl Request for GetTransactions {
    type Response = BlockTransactionsExt;
}

#[derive(Debug, Clone, Default)]
pub struct Sync {}

impl Request for Sync {
    type Response = BlockIdExt;
}

#[derive(Debug, Clone)]
pub struct SendMessage {
    pub body: String,
}

impl Request for SendMessage {
    type Response = ();
}

#[derive(Debug, Clone)]
pub struct SendMessageReturningHash {
    pub body: String,
}

impl Request for SendMessageReturningHash {
    type Response = String;
}

#[derive(Debug, Clone)]
pub struct GetAccountState {
    pub address: SmartContractAddress,
}

impl Request for GetAccountState {
    type Response = AccountState;
}

#[derive(Debug, Clone)]
pub struct GetAccountStateOnBlock {
    pub address: SmartContractAddress,
    pub block_id: BlockIdExt,
}

impl Request for GetAccountStateOnBlock {
    type Response = AccountState;
}

#[derive(Debug, Clone)]
pub struct GetAccountStateByTransaction {
    pub address: SmartContractAddress,
    pub transaction_id: TransactionId,
}

impl Request for GetAccountStateByTransaction {
    type Response = AccountState;
}

#[derive(Debug, Clone)]
pub struct GetAccountTransactions {
    pub address: SmartContractAddress,
    pub from: TransactionId,
}

impl Request for GetAccountTransactions {
    type Response = Transactions;
}

#[derive(Debug, Clone)]
pub struct GetShardAccountCell {
    pub address: SmartContractAddress,
}

impl Request for GetShardAccountCell {
    type Response = Cell;
}

#[derive(Debug, Clone)]
pub struct GetShardAccountCellOnBlock {
    pub address: SmartContractAddress,
    pub block_id: BlockIdExt,
}

impl Request for GetShardAccountCellOnBlock {
    type Response = Cell;
}

#[derive(Debug, Clone)]
pub struct GetShardAccountCellByTransaction {
    pub address: SmartContractAddress,
    pub transaction_id: TransactionId,
}

impl Request for GetShardAccountCellByTransaction {
    type Response = Cell;
}

#[derive(Debug, Clone)]
pub struct RunGetMethod {
    pub address: SmartContractAddress,
    pub method: String,
    pub stack: Vec<StackEntry>,
}

impl Request for RunGetMethod {
    type Response = SmcRunResult;
}
