mod actor;
mod builder;
pub mod client;
pub mod pool;
pub mod route;
pub mod service;

use crate::pool::Forward;
use ton_tower::{Request, request::*};
use tower::Service;

pub use builder::{PoolTransport, TonClientBuilder};
pub use client::*;
pub use route::*;

pub trait RequestHandler<R>:
    Service<R, Response = R::Response, Error = anyhow::Error, Future: Send> + Send
where
    R: Request,
{
}

impl<R: Request, T> RequestHandler<R> for T where
    T: Service<R, Response = R::Response, Error = anyhow::Error, Future: Send> + Send
{
}

pub trait ForwardHandler<R: Request>:
    Service<Forward<R>, Response = R::Response, Error = anyhow::Error, Future: Send> + Send
{
}

impl<R: Request, T> ForwardHandler<R> for T where
    T: Service<Forward<R>, Response = R::Response, Error = anyhow::Error, Future: Send> + Send
{
}

macro_rules! define_ton_service {
    ($($req:ty),+ $(,)?) => {
        pub trait TonService:
            $(RequestHandler<$req> +)+
            Clone + Send + std::marker::Sync + 'static
        {
        }

        impl<T> TonService for T where
            T: $(RequestHandler<$req> +)+
                Clone + Send + std::marker::Sync + 'static
        {
        }

        pub trait TonPoolService:
            TonService
            $(+ ForwardHandler<$req>)+
        {
        }

        impl<T> TonPoolService for T where
            T: TonService
                $(+ ForwardHandler<$req>)+
        {
        }
    };
}

define_ton_service! {
    GetMasterchainInfo,
    Sync,
    LookUpBlockBySeqno,
    LookUpBlockByLt,
    GetShards,
    GetBlockHeader,
    GetTransactionIds,
    GetTransactions,
    GetAccountState,
    GetAccountStateOnBlock,
    GetAccountStateByTransaction,
    GetAccountTransactions,
    GetShardAccountCell,
    GetShardAccountCellOnBlock,
    GetShardAccountCellByTransaction,
    RunGetMethod,
    SendMessage,
    SendMessageReturningHash,
}
