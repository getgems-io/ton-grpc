use std::pin::Pin;
use anyhow::anyhow;
use async_stream::stream;
use futures::{StreamExt, Stream};
use tonic::{async_trait, Request, Response, Status, Streaming};
use tracing::{error};
use crate::threaded::{Command, Stop};
use crate::tvm::tvm_emulator_request::Request::{Prepare, RunGetMethod, SendExternalMessage, SendInternalMessage, SetC7, SetGasLimit, SetLibraries};
use crate::tvm::tvm_emulator_response::Response::{PrepareResponse, RunGetMethodResponse, SendExternalMessageResponse, SendInternalMessageResponse, SetC7Response, SetGasLimitResponse, SetLibrariesResponse};
use crate::tvm::tvm_emulator_service_server::TvmEmulatorService as BaseTvmEmulatorService;
use crate::tvm::{tvm_emulator_request, tvm_emulator_response, TvmEmulatorPrepareRequest, TvmEmulatorPrepareResponse, TvmEmulatorRequest, TvmEmulatorResponse, TvmEmulatorRunGetMethodRequest, TvmEmulatorRunGetMethodResponse, TvmEmulatorSendExternalMessageRequest, TvmEmulatorSendExternalMessageResponse, TvmEmulatorSendInternalMessageRequest, TvmEmulatorSendInternalMessageResponse, TvmEmulatorSetC7Request, TvmEmulatorSetC7Response, TvmEmulatorSetGasLimitRequest, TvmEmulatorSetGasLimitResponse, TvmEmulatorSetLibrariesRequest, TvmEmulatorSetLibrariesResponse, TvmResult};

#[derive(Debug, Default)]
pub struct TvmEmulatorService;

#[derive(Default)]
struct State {
    emulator: Option<tonlibjson_sys::TvmEmulator>
}

#[async_trait]
impl BaseTvmEmulatorService for TvmEmulatorService {
    type ProcessStream = Pin<Box<dyn Stream<Item=Result<TvmEmulatorResponse, Status>> + Send>>;

    async fn process(&self, request: Request<Streaming<TvmEmulatorRequest>>) -> Result<Response<Self::ProcessStream>, Status> {
        let stream = request.into_inner();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Command<tvm_emulator_request::Request, tvm_emulator_response::Response>>();
        let stop = Stop::new(tx.clone());

        rayon::spawn(move || {
            let mut state = State::default();
            while let Some(command) = rx.blocking_recv() {
                match command {
                    Command::Request { request, response: oneshot } => {
                        let response = match request {
                            Prepare(req) => prepare_emu(&mut state, req).map(PrepareResponse),
                            RunGetMethod(req) => run_get_method(&mut state, req).map(RunGetMethodResponse),
                            SendExternalMessage(req) => send_external_message(&mut state, req).map(SendExternalMessageResponse),
                            SendInternalMessage(req) => send_internal_message(&mut state, req).map(SendInternalMessageResponse),
                            SetLibraries(req) => set_libraries(&mut state, req).map(SetLibrariesResponse),
                            SetGasLimit(req) => set_gas_limit(&mut state, req).map(SetGasLimitResponse),
                            SetC7(req) => set_c7(&mut state, req).map(SetC7Response),
                        };

                        oneshot.send(response).expect("failed to send response");
                    },
                    Command::Drop => { break; }
                }
            }
        });

        let output = stream! {
            for await msg in stream {
                match msg {
                    Ok(TvmEmulatorRequest { request_id, request: Some(req)}) => {
                        let (to, ro) = tokio::sync::oneshot::channel();

                        let _ = tx.send(Command::Request { request: req, response: to });
                        let response = ro.await.expect("failed to receive response");

                        yield response
                            .map(|r| TvmEmulatorResponse { request_id, response: Some(r) })
                            .map_err(|e| {
                                error!(error = ?e);
                                Status::internal(e.to_string())
                            })
                    },
                    Ok(TvmEmulatorRequest{ request_id, request: None }) => {
                        tracing::error!(error = ?anyhow!("empty request"), request_id=request_id);

                        break
                    },
                    Err(e) =>  {
                        tracing::error!(error = ?e);

                        break
                    }
                }
            }

            drop(stop);
        }.boxed();

        Ok(Response::new(output))
    }
}

fn prepare_emu(state: &mut State, req: TvmEmulatorPrepareRequest) -> anyhow::Result<TvmEmulatorPrepareResponse> {
    state.emulator.replace(tonlibjson_sys::TvmEmulator::new(&req.code_boc, &req.data_boc, req.vm_log_verbosity)?);

    Ok(TvmEmulatorPrepareResponse { success: true })
}

fn run_get_method(state: &mut State, req: TvmEmulatorRunGetMethodRequest) -> anyhow::Result<TvmEmulatorRunGetMethodResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.run_get_method(req.method_id, &req.stack_boc)?;
    tracing::trace!(method="run_get_method", "{}", response);

    let response = serde_json::from_str::<TvmResult<TvmEmulatorRunGetMethodResponse>>(&response)?;

    response.into()
}

fn send_external_message(state: &mut State, req: TvmEmulatorSendExternalMessageRequest) -> anyhow::Result<TvmEmulatorSendExternalMessageResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.send_external_message(&req.message_body_boc)?;
    tracing::trace!(method="send_external_message", "{}", response);

    let response = serde_json::from_str::<TvmResult<TvmEmulatorSendExternalMessageResponse>>(&response)?;

    response.into()
}

fn send_internal_message(state: &mut State, req: TvmEmulatorSendInternalMessageRequest) -> anyhow::Result<TvmEmulatorSendInternalMessageResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.send_internal_message(&req.message_body_boc, req.amount)?;
    tracing::trace!(method="send_internal_message", "{}", response);

    let response = serde_json::from_str::<TvmResult<TvmEmulatorSendInternalMessageResponse>>(&response)?;

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
