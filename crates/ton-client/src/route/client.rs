use crate::RequestHandler;
use crate::route::discover::first_block::FirstBlockDiscoverActorHandle;
use crate::route::discover::last_block::LastBlockDiscoverActorHandle;
use crate::route::registry::Registry;
use crate::route::{BlockCriteria, ChainId, Routed, Seqno};
use futures::future::{Ready, ready};
use std::convert::Infallible;
use std::sync::Arc;
use std::task::{Context, Poll};
use ton_tower::{Request, request::*};
use tower::Service;
use tower::load::Load;

#[derive(Clone)]
pub struct RoutedClient<S> {
    client: S,
    _first_block_handle: FirstBlockDiscoverActorHandle,
    last_block_handle: LastBlockDiscoverActorHandle,
    registry: Arc<Registry>,
}

impl<S> Routed for RoutedClient<S> {
    fn contains(&self, chain: &ChainId, criteria: &BlockCriteria) -> bool {
        self.registry.contains(chain, criteria, false)
    }

    fn contains_not_available(&self, chain: &ChainId, criteria: &BlockCriteria) -> bool {
        self.registry.contains(chain, criteria, true)
    }

    fn last_seqno(&self) -> Option<Seqno> {
        let master_shard_id = self
            .last_block_handle
            .last_value()
            .as_ref()
            .map(|info| (info.last.workchain, info.last.shard))?;

        self.registry.get_last_seqno(&master_shard_id)
    }
}

impl<S> RoutedClient<S>
where
    S: RequestHandler<GetMasterchainInfo>
        + RequestHandler<Sync>
        + RequestHandler<LookUpBlockBySeqno>
        + RequestHandler<GetBlockHeader>
        + RequestHandler<GetShards>
        + Load
        + Clone
        + Send
        + std::marker::Sync
        + 'static,
    S::Metric: Into<f64>,
{
    pub fn new(id: String, client: S) -> Self {
        metrics::describe_counter!(
            "ton_liteserver_last_seqno",
            "The seqno of the latest block that is available for the liteserver to sync"
        );
        metrics::describe_counter!(
            "ton_liteserver_synced_seqno",
            "The seqno of the last block with which the liteserver is actually synchronized"
        );
        metrics::describe_counter!(
            "ton_liteserver_first_seqno",
            "The seqno of the first block that is available for the liteserver to request"
        );
        metrics::describe_gauge!("ton_liteserver_requests_total", "Total count of requests");
        metrics::describe_gauge!("ton_liteserver_requests", "Number of concurrent requests");

        let registry = Arc::new(Registry::default());

        let last_block_handle =
            LastBlockDiscoverActorHandle::new(id.clone(), registry.clone(), client.clone());

        let first_block_handle = FirstBlockDiscoverActorHandle::new(
            id.clone(),
            registry.clone(),
            client.clone(),
            last_block_handle.clone(),
        );

        Self {
            client,
            _first_block_handle: first_block_handle,
            last_block_handle,
            registry,
        }
    }
}

impl<S> RoutedClient<S> {
    fn edges_defined(&self) -> bool {
        let Some(master_shard_id) = self
            .last_block_handle
            .last_value()
            .as_ref()
            .map(|info| (info.last.workchain, info.last.shard))
        else {
            return false;
        };

        self.registry.edges_defined(&master_shard_id)
    }
}

impl<S> Service<GetMasterchainInfo> for RoutedClient<S> {
    type Response = <GetMasterchainInfo as Request>::Response;
    type Error = Infallible;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.edges_defined() {
            Poll::Ready(Ok(()))
        } else {
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    fn call(&mut self, _: GetMasterchainInfo) -> Self::Future {
        let response = self
            .last_block_handle
            .last_value()
            .as_ref()
            .expect("called before ready")
            .clone();

        ready(Ok(response))
    }
}

macro_rules! forward_service {
    ($($req:ty),* $(,)?) => {
        $(
            impl<S> Service<$req> for RoutedClient<S>
            where
                S: Service<$req>,
            {
                type Response = <S as Service<$req>>::Response;
                type Error = <S as Service<$req>>::Error;
                type Future = <S as Service<$req>>::Future;

                fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                    if self.edges_defined() {
                        return self.client.poll_ready(cx);
                    }

                    cx.waker().wake_by_ref();

                    Poll::Pending
                }

                fn call(&mut self, req: $req) -> Self::Future {
                    self.client.call(req)
                }
            }
        )*
    };
}

forward_service!(
    Sync,
    LookUpBlockBySeqno,
    GetBlockHeader,
    GetShards,
    LookUpBlockByLt,
    GetTransactionIds,
    GetTransactions,
    SendMessage,
    SendMessageReturningHash,
    GetAccountState,
    GetAccountStateOnBlock,
    GetAccountStateByTransaction,
    GetAccountTransactions,
    GetShardAccountCell,
    GetShardAccountCellOnBlock,
    GetShardAccountCellByTransaction,
    RunGetMethod,
);

impl<S> Load for RoutedClient<S>
where
    S: Load,
{
    type Metric = S::Metric;

    fn load(&self) -> Self::Metric {
        self.client.load()
    }
}
