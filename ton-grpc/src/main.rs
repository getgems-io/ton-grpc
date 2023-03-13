#[derive(Debug, Default)]
struct TvmEmulatorService;

mod tvm_emulator {
    tonic::include_proto!("ton");

    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("ton_descriptor");
}

use std::pin::Pin;
use anyhow::anyhow;
use tonic::{async_trait, Request, Response, Status, Streaming};
use futures::{Stream, StreamExt};
use futures::future::ready;
use serde::Deserialize;
use tonic::transport::Server;
use crate::tvm_emulator::{TvmEmulatorPrepareResponse, TvmEmulatorRequest, TvmEmulatorResponse, TvmEmulatorRunGetMethodRequest, TvmEmulatorRunGetMethodResponse};
use crate::tvm_emulator::tvm_emulator_server::{TvmEmulator, TvmEmulatorServer};
use crate::tvm_emulator::tvm_emulator_request::Request::{Prepare, RunGetMethod};
use crate::tvm_emulator::tvm_emulator_response::Response::{Prepare as PrepareResponse, RunGetMethod as RunGetMethodResponse};

struct State {
    emulator: Option<tonlibjson_sys::TvmEmulator>
}

#[derive(Deserialize)]
struct TvmResult<T> {
    pub success: bool,
    pub error: Option<String>,
    #[serde(flatten)]
    pub data: Option<T>
}

#[async_trait]
impl TvmEmulator for TvmEmulatorService {
    type ProcessStream = Pin<Box<dyn Stream<Item=Result<TvmEmulatorResponse, Status>> + Send>>;

    async fn process(&self, request: Request<Streaming<TvmEmulatorRequest>>) -> Result<Response<Self::ProcessStream>, Status> {
        let stream = request.into_inner();

        let output = stream.scan(State { emulator: None }, |state, msg| {
            match msg {
                Ok(TvmEmulatorRequest { request: Some(Prepare(prepare))}) => {
                    let Ok(emulator) = tonlibjson_sys::TvmEmulator::new(&prepare.code_boc, &prepare.data_boc, 1) else {
                        return ready(Some(Err(Status::internal("cannot create emulator"))));
                    };

                    (*state).emulator = Some(emulator);

                    ready(Some(Ok(TvmEmulatorResponse { response: Some(PrepareResponse(TvmEmulatorPrepareResponse { success: true })) })))
                },
                Ok(TvmEmulatorRequest { request: Some(RunGetMethod(req))}) => {
                    let response = run_get_method(state, req);

                    match response {
                        Ok(response) => ready(Some(Ok(TvmEmulatorResponse { response: Some(RunGetMethodResponse(response)) }))),
                        Err(e) => ready(Some(Err(Status::internal(e.to_string()))))
                    }
                }
                _ => ready(None)
            }
        }).boxed();

        Ok(Response::new(output))
    }
}

fn run_get_method(state: &mut State, req: TvmEmulatorRunGetMethodRequest) -> anyhow::Result<TvmEmulatorRunGetMethodResponse> {
    let Some(emu) = state.emulator.as_ref().take() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.run_get_method(req.method_id, &req.stack_boc)?;
    let response = serde_json::from_str::<TvmResult<TvmEmulatorRunGetMethodResponse>>(response)?;

    return if response.success {
        Ok(response.data.unwrap_or_default())
    } else {
        Err(anyhow!(response.error.unwrap_or("ambiguous response".to_owned())))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(tvm_emulator::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    let addr = "127.0.0.1:50052".parse().unwrap();

    let route_guide = TvmEmulatorService::default();

    let svc = TvmEmulatorServer::new(route_guide);

    Server::builder()
        .add_service(reflection)
        .add_service(svc)
        .serve(addr)
        .await?;

    Ok(())
}
