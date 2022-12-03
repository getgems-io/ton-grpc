mod grpc;

use tonic::{Request, Response, Status};
use tonic::transport::Server;
use tonlibjson_client::ton::TonClient;
use crate::grpc::ton_server::{Ton, TonServer};
use crate::grpc::{GetMasterchainInfoRequest, GetMasterchainInfoResponse};

struct TonService {
    client: TonClient
}

#[tonic::async_trait]
impl Ton for TonService {
    async fn get_masterchain_info(&self, _: Request<GetMasterchainInfoRequest>) -> Result<Response<GetMasterchainInfoResponse>, Status> {
        let masterchain_info = self.client.get_masterchain_info().await
            .map_err(|e| Status::internal(format!("{}", e)))?;

        Ok(Response::new(masterchain_info.into()))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let addr = "[::1]:3009".parse().unwrap();

    let ton = TonService {
        client: TonClient::from_env().await?
    };

    let svc = TonServer::new(ton);

    Server::builder()
        .accept_http1(true)
        .add_service(tonic_web::enable(svc))
        .serve(addr)
        .await?;

    Ok(())
}
