use std::future::Future;
use std::ops::Bound;

use futures::stream::BoxStream;

use crate::types::{
    BlockIdExt, BlocksHeader, InternalAccountAddress, InternalTransactionId, MasterchainInfo,
    RawFullAccountState, RawTransaction, ShortTxId, TvmCell,
};

pub trait TonClient: Clone + Send + Sync + 'static {
    type Error: Send + Sync + 'static;

    fn ready(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn get_masterchain_info(
        &self,
    ) -> impl Future<Output = Result<MasterchainInfo, Self::Error>> + Send;

    fn look_up_block_by_seqno(
        &self,
        workchain: i32,
        shard: i64,
        seqno: i32,
    ) -> impl Future<Output = Result<BlockIdExt, Self::Error>> + Send;

    fn get_block_header(
        &self,
        workchain: i32,
        shard: i64,
        seqno: i32,
        hashes: Option<(String, String)>,
    ) -> impl Future<Output = Result<BlocksHeader, Self::Error>> + Send;

    fn get_shards_by_block_id(
        &self,
        block_id: BlockIdExt,
    ) -> impl Future<Output = Result<Vec<BlockIdExt>, Self::Error>> + Send;

    fn raw_get_account_state_on_block(
        &self,
        address: &str,
        block_id: BlockIdExt,
    ) -> impl Future<Output = Result<RawFullAccountState, Self::Error>> + Send;

    fn raw_get_account_state_at_least_block(
        &self,
        address: &str,
        block_id: &BlockIdExt,
    ) -> impl Future<Output = Result<RawFullAccountState, Self::Error>> + Send;

    fn raw_get_account_state_by_transaction(
        &self,
        address: &str,
        transaction_id: InternalTransactionId,
    ) -> impl Future<Output = Result<RawFullAccountState, Self::Error>> + Send;

    fn get_shard_account_cell_on_block(
        &self,
        address: &str,
        block: BlockIdExt,
    ) -> impl Future<Output = Result<TvmCell, Self::Error>> + Send;

    fn get_shard_account_cell_at_least_block(
        &self,
        address: &str,
        block_id: &BlockIdExt,
    ) -> impl Future<Output = Result<TvmCell, Self::Error>> + Send;

    fn send_message_returning_hash(
        &self,
        body: &str,
    ) -> impl Future<Output = Result<String, Self::Error>> + Send;

    fn get_account_tx_range_unordered(
        &self,
        address: &str,
        range: (Bound<InternalTransactionId>, Bound<InternalTransactionId>),
    ) -> impl Future<
        Output = Result<BoxStream<'static, Result<RawTransaction, Self::Error>>, Self::Error>,
    > + Send;

    fn get_account_tx_range(
        &self,
        address: &str,
        range: (Bound<InternalTransactionId>, Bound<InternalTransactionId>),
    ) -> BoxStream<'static, Result<RawTransaction, Self::Error>>;

    fn get_block_tx_stream_unordered(
        &self,
        block: &BlockIdExt,
    ) -> BoxStream<'static, Result<ShortTxId, Self::Error>>;

    fn get_block_tx_id_stream(
        &self,
        block: &BlockIdExt,
        reverse: bool,
    ) -> BoxStream<'static, Result<ShortTxId, Self::Error>>;

    fn get_block_tx_stream(
        &self,
        block: &BlockIdExt,
        reverse: bool,
    ) -> BoxStream<'static, Result<RawTransaction, Self::Error>>;

    fn get_accounts_in_block_stream(
        &self,
        block: &BlockIdExt,
    ) -> BoxStream<'static, Result<InternalAccountAddress, Self::Error>>;
}
