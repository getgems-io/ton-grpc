use crate::block::{BlocksGetMasterchainInfo, BlocksMasterchainInfo};
use crate::client::Client;
use crate::cursor::discover::first_block_discover::FirstBlockDiscover;
use crate::cursor::discover::last_block_discover::LastBlockDiscover;
use crate::cursor::registry::Registry;
use crate::cursor::{ChainId, Seqno};
use crate::error::ErrorService;
use crate::metric::ConcurrencyMetric;
use crate::request::Specialized;
use anyhow::Result;
use futures::future::ready;
use futures::FutureExt;
use std::borrow::Cow;
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::watch::{Receiver, Sender};
use ton_client_util::router::route::BlockCriteria;
use ton_client_util::router::Routed;
use ton_client_util::service::shared::SharedService;
use ton_client_util::service::timeout::Timeout;
use tower::limit::ConcurrencyLimit;
use tower::load::peak_ewma::Cost;
use tower::load::Load;
use tower::load::PeakEwma;
use tower::Service;

pub type InnerClient =
    ConcurrencyMetric<ConcurrencyLimit<SharedService<ErrorService<Timeout<PeakEwma<Client>>>>>>;

#[derive(Clone)]
pub(crate) struct CursorClient {
    id: Cow<'static, str>,
    client: InnerClient,

    masterchain_info_rx: Receiver<Option<BlocksMasterchainInfo>>,
    registry: Arc<Registry>,
}

impl Routed for CursorClient {
    fn contains(&self, chain: &ChainId, criteria: &BlockCriteria) -> bool {
        self.registry.contains(chain, criteria, false)
    }

    fn contains_not_available(&self, chain: &ChainId, criteria: &BlockCriteria) -> bool {
        self.registry.contains(chain, criteria, true)
    }

    fn last_seqno(&self) -> Option<Seqno> {
        let master_shard_id = self
            .masterchain_info_rx
            .borrow()
            .as_ref()
            .map(|info| (info.last.workchain, info.last.shard))?;

        self.registry.get_last_seqno(&master_shard_id)
    }
}

impl CursorClient {
    pub(crate) fn new(
        id: String,
        client: ConcurrencyLimit<SharedService<ErrorService<Timeout<PeakEwma<Client>>>>>,
    ) -> Self {
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

        let id = Cow::from(id);
        let client = ConcurrencyMetric::new(client, id.clone());
        let (mtx, mrx) = tokio::sync::watch::channel(None);
        let mut mc_watcher = mtx.subscribe();

        let _self = Self {
            id,
            client,

            masterchain_info_rx: mrx,
            registry: Default::default(),
        };

        tokio::spawn(_self.last_block_loop(mtx));
        let inner = _self.first_block_loop();
        tokio::spawn(async move {
            mc_watcher.changed().await.unwrap();

            inner.await;
        });

        _self
    }

    fn last_block_loop(
        &self,
        mtx: Sender<Option<BlocksMasterchainInfo>>,
    ) -> impl Future<Output = Infallible> {
        let id = self.id.clone();
        let client = self.client.clone();
        let registry = self.registry.clone();

        let discover = LastBlockDiscover::new(id, client, registry, mtx);

        discover.discover()
    }

    fn first_block_loop(&self) -> impl Future<Output = Infallible> {
        let id = self.id.clone();
        let client = self.client.clone();
        let registry = self.registry.clone();

        let discover =
            FirstBlockDiscover::new(id, client, registry, self.masterchain_info_rx.clone());

        discover.discover()
    }

    fn edges_defined(&self) -> bool {
        let Some(master_shard_id) = self
            .masterchain_info_rx
            .borrow()
            .as_ref()
            .map(|info| (info.last.workchain, info.last.shard))
        else {
            return false;
        };

        self.registry.edges_defined(&master_shard_id)
    }
}

impl Service<Specialized<BlocksGetMasterchainInfo>> for CursorClient {
    type Response = BlocksMasterchainInfo;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        if self.masterchain_info_rx.borrow().is_some() && self.edges_defined() {
            Poll::Ready(Ok(()))
        } else {
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    fn call(&mut self, _: Specialized<BlocksGetMasterchainInfo>) -> Self::Future {
        let response = self.masterchain_info_rx.borrow().as_ref().unwrap().clone();

        ready(Ok(response)).boxed()
    }
}

impl<R> Service<R> for CursorClient
where
    InnerClient: Service<R>,
{
    type Response = <InnerClient as Service<R>>::Response;
    type Error = <InnerClient as Service<R>>::Error;
    type Future = <InnerClient as Service<R>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        if self.edges_defined() {
            return self.client.poll_ready(cx);
        }

        cx.waker().wake_by_ref();

        Poll::Pending
    }

    fn call(&mut self, req: R) -> Self::Future {
        self.client.call(req)
    }
}

impl Load for CursorClient {
    type Metric = Cost;

    fn load(&self) -> Self::Metric {
        self.client.get_ref().get_ref().load()
    }
}
