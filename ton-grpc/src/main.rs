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
use crate::tvm_emulator::{TvmEmulatorPrepareRequest, TvmEmulatorPrepareResponse, TvmEmulatorRequest, TvmEmulatorResponse, TvmEmulatorRunGetMethodRequest, TvmEmulatorRunGetMethodResponse, TvmEmulatorSendExternalMessageRequest, TvmEmulatorSendExternalMessageResponse, TvmEmulatorSendInternalMessageRequest, TvmEmulatorSendInternalMessageResponse, TvmEmulatorSetC7Request, TvmEmulatorSetC7Response, TvmEmulatorSetGasLimitRequest, TvmEmulatorSetGasLimitResponse, TvmEmulatorSetLibrariesRequest, TvmEmulatorSetLibrariesResponse};
use crate::tvm_emulator::tvm_emulator_server::{TvmEmulator, TvmEmulatorServer};
use crate::tvm_emulator::tvm_emulator_request::Request::{Prepare, RunGetMethod, SendExternalMessage, SendInternalMessage, SetC7, SetGasLimit, SetLibraries};
use crate::tvm_emulator::tvm_emulator_response::Response::{PrepareResponse, RunGetMethodResponse, SendExternalMessageResponse, SendInternalMessageResponse, SetC7Response, SetGasLimitResponse, SetLibrariesResponse};

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

impl<T> Into<anyhow::Result<T>> for TvmResult<T> where T: Default {
    fn into(self) -> anyhow::Result<T> {
        return if self.success {
            Ok(self.data.unwrap_or_default())
        } else {
            Err(anyhow!(self.error.unwrap_or("ambiguous response".to_owned())))
        }
    }
}

#[async_trait]
impl TvmEmulator for TvmEmulatorService {
    type ProcessStream = Pin<Box<dyn Stream<Item=Result<TvmEmulatorResponse, Status>> + Send>>;

    async fn process(&self, request: Request<Streaming<TvmEmulatorRequest>>) -> Result<Response<Self::ProcessStream>, Status> {
        let stream = request.into_inner();

        let output = stream.scan(State { emulator: None }, |state, msg| {
            match msg {
                Ok(TvmEmulatorRequest { request: Some(req)}) => {
                    let response = match req {
                        Prepare(req) => prepare_emu(state, req).map(PrepareResponse),
                        RunGetMethod(req) => run_get_method(state, req).map(RunGetMethodResponse),
                        SendExternalMessage(req) => send_external_message(state, req).map(SendExternalMessageResponse),
                        SendInternalMessage(req) => send_internal_message(state, req).map(SendInternalMessageResponse),
                        SetLibraries(req) => set_libraries(state, req).map(SetLibrariesResponse),
                        SetGasLimit(req) => set_gas_limit(state, req).map(SetGasLimitResponse),
                        SetC7(req) => set_c7(state, req).map(SetC7Response)
                    };

                    ready(Some(response
                        .map(|r| TvmEmulatorResponse { response: Some(r) })
                        .map_err(|e| Status::internal(e.to_string()))))

                },
                Ok(TvmEmulatorRequest{ request: None }) => {
                    tracing::error!(error = ?anyhow!("empty request"));

                    ready(None)
                },
                Err(e) =>  {
                    tracing::error!(error = ?e);

                    ready(None)
                }
            }
        }).boxed();

        Ok(Response::new(output))
    }
}

fn prepare_emu(state: &mut State, req: TvmEmulatorPrepareRequest) -> anyhow::Result<TvmEmulatorPrepareResponse> {
    if let Ok(emulator) = tonlibjson_sys::TvmEmulator::new(&req.code_boc, &req.data_boc, 1) {
        state.emulator = Some(emulator);

        Ok(TvmEmulatorPrepareResponse { success: true })
    } else {
        Err(anyhow!("cannot create emulator"))
    }
}

fn run_get_method(state: &mut State, req: TvmEmulatorRunGetMethodRequest) -> anyhow::Result<TvmEmulatorRunGetMethodResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.run_get_method(req.method_id, &req.stack_boc)?;
    tracing::trace!(method="run_get_method", "{}", response);

    let response = serde_json::from_str::<TvmResult<TvmEmulatorRunGetMethodResponse>>(response)?;

    response.into()
}

fn send_external_message(state: &mut State, req: TvmEmulatorSendExternalMessageRequest) -> anyhow::Result<TvmEmulatorSendExternalMessageResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.send_external_message(&req.message_body_boc)?;
    tracing::trace!(method="send_external_message", "{}", response);

    let response = serde_json::from_str::<TvmResult<TvmEmulatorSendExternalMessageResponse>>(response)?;

    response.into()
}

fn send_internal_message(state: &mut State, req: TvmEmulatorSendInternalMessageRequest) -> anyhow::Result<TvmEmulatorSendInternalMessageResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.send_internal_message(&req.message_body_boc, req.amount)?;
    tracing::trace!(method="send_internal_message", "{}", response);

    let response = serde_json::from_str::<TvmResult<TvmEmulatorSendInternalMessageResponse>>(response)?;

    response.into()
}

fn set_libraries(state: &mut State, req: TvmEmulatorSetLibrariesRequest) -> anyhow::Result<TvmEmulatorSetLibrariesResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.set_libraries(&req.libs_boc)?;
    tracing::trace!(method="set_libraries", "{}", response);

    Ok(TvmEmulatorSetLibrariesResponse { success: response })
}

fn set_gas_limit(state: &mut State, req: TvmEmulatorSetGasLimitRequest) -> anyhow::Result<TvmEmulatorSetGasLimitResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.set_gas_limit(req.gas_limit);
    tracing::trace!(method="set_gas_limit", "{}", response);

    Ok(TvmEmulatorSetGasLimitResponse { success: response })
}

fn set_c7(state: &mut State, req: TvmEmulatorSetC7Request) -> anyhow::Result<TvmEmulatorSetC7Response> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.set_c7(&req.address, req.unixtime, req.balance, &req.rand_seed_hex, &req.config)?;
    tracing::trace!(method="set_c7", "{}", response);

    Ok(TvmEmulatorSetC7Response { success: response })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tonlibjson_sys::TvmEmulator::set_verbosity_level(0);

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
