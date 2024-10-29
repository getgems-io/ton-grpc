use crate::threaded::{Command, Stop, StreamId};
use crate::tvm::tvm_emulator_request::Request::{
    Prepare, RunGetMethod, SendExternalMessage, SendInternalMessage, SetC7, SetGasLimit,
    SetLibraries,
};
use crate::tvm::tvm_emulator_response::Response::{
    PrepareResponse, RunGetMethodResponse, SendExternalMessageResponse,
    SendInternalMessageResponse, SetC7Response, SetGasLimitResponse, SetLibrariesResponse,
};
use crate::tvm::tvm_emulator_service_server::TvmEmulatorService as BaseTvmEmulatorService;
use crate::tvm::{
    tvm_emulator_request, tvm_emulator_response, RunGetMethodRequest, TvmEmulatorPrepareRequest,
    TvmEmulatorPrepareResponse, TvmEmulatorRequest, TvmEmulatorResponse,
    TvmEmulatorRunGetMethodRequest, TvmEmulatorRunGetMethodResponse,
    TvmEmulatorSendExternalMessageRequest, TvmEmulatorSendExternalMessageResponse,
    TvmEmulatorSendInternalMessageRequest, TvmEmulatorSendInternalMessageResponse,
    TvmEmulatorSetC7Request, TvmEmulatorSetC7Response, TvmEmulatorSetGasLimitRequest,
    TvmEmulatorSetGasLimitResponse, TvmEmulatorSetLibrariesRequest,
    TvmEmulatorSetLibrariesResponse, TvmResult,
};
use anyhow::anyhow;
use async_stream::stream;
use futures::Stream;
use quick_cache::sync::Cache;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::StreamExt;
use tonic::{async_trait, Request, Response, Status, Streaming};
use tonlibjson_sys::emulate_run_method;
use tracing::instrument;

#[derive(Debug)]
pub struct TvmEmulatorService {
    tx: UnboundedSender<Command<tvm_emulator_request::Request, tvm_emulator_response::Response>>,
}

impl Default for TvmEmulatorService {
    fn default() -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<
            Command<tvm_emulator_request::Request, tvm_emulator_response::Response>,
        >();

        tokio::task::spawn_blocking(move || {
            let mut states: HashMap<StreamId, State> = HashMap::default();

            while let Some(command) = rx.blocking_recv() {
                match command {
                    Command::Request {
                        stream_id,
                        request,
                        response: oneshot,
                    } => {
                        let state = states.entry(stream_id).or_default();

                        let response = match request {
                            Prepare(req) => prepare_emu(state, req).map(PrepareResponse),
                            RunGetMethod(req) => {
                                run_get_method(state, req).map(RunGetMethodResponse)
                            }
                            SendExternalMessage(req) => {
                                send_external_message(state, req).map(SendExternalMessageResponse)
                            }
                            SendInternalMessage(req) => {
                                send_internal_message(state, req).map(SendInternalMessageResponse)
                            }
                            SetLibraries(req) => {
                                set_libraries(state, req).map(SetLibrariesResponse)
                            }
                            SetGasLimit(req) => set_gas_limit(state, req).map(SetGasLimitResponse),
                            SetC7(req) => set_c7(state, req).map(SetC7Response),
                        };

                        if let Err(e) = oneshot.send(response) {
                            tracing::error!(error = ?e, "failed to send response");
                            states.remove(&stream_id);
                        }
                    }
                    Command::Drop { stream_id } => {
                        states.remove(&stream_id);
                    }
                }
            }
        });

        Self { tx }
    }
}

#[derive(Default)]
struct State {
    emulator: Option<tonlibjson_sys::TvmEmulator>,
}

fn lru_cache() -> &'static Cache<String, Arc<String>> {
    static LRU_CACHE: OnceLock<Cache<String, Arc<String>>> = OnceLock::new();

    LRU_CACHE.get_or_init(|| Cache::new(32))
}

#[async_trait]
impl BaseTvmEmulatorService for TvmEmulatorService {
    type ProcessStream = Pin<Box<dyn Stream<Item = Result<TvmEmulatorResponse, Status>> + Send>>;

    async fn process(
        &self,
        request: Request<Streaming<TvmEmulatorRequest>>,
    ) -> Result<Response<Self::ProcessStream>, Status> {
        let stream_id = StreamId::new_v4();
        let stream = request.into_inner();
        let stream = stream.timeout(Duration::from_secs(3));
        let stop = Stop::new(stream_id, self.tx.clone());
        let tx = self.tx.clone();

        let output = stream! {
            for await msg in stream {
                match msg {
                    Ok(Ok(TvmEmulatorRequest { request_id, request: Some(req)})) => {
                        let (to, ro) = tokio::sync::oneshot::channel();

                        let _ = tx.send(Command::Request { stream_id, request: req, response: to });
                        let Ok(response) = ro.await else {
                            break
                        };

                        yield response.map(|r| TvmEmulatorResponse { request_id, response: Some(r) })
                    },
                    Ok(Ok(TvmEmulatorRequest{ request_id, request: None })) => {
                        tracing::error!(error = ?anyhow!("empty request"), request_id=request_id);

                        break
                    },
                    Ok(Err(e)) =>  {
                        tracing::error!(error = ?e);

                        break
                    },
                    Err(e) =>  {
                        tracing::error!(error = ?e);

                        break
                    }
                }
            }

            drop(stop);
        };

        Ok(Response::new(Box::pin(output)))
    }

    async fn run_get_method(
        &self,
        request: Request<RunGetMethodRequest>,
    ) -> Result<Response<crate::tvm::RunGetMethodResponse>, Status> {
        let req = request.into_inner();

        let result = emulate_run_method(&req.params_boc, req.gas_limit)
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(crate::tvm::RunGetMethodResponse { result }))
    }
}

fn prepare_emu(
    state: &mut State,
    req: TvmEmulatorPrepareRequest,
) -> Result<TvmEmulatorPrepareResponse, Status> {
    let emulator =
        tonlibjson_sys::TvmEmulator::new(&req.code_boc, &req.data_boc, req.vm_log_verbosity)
            .map_err(|e| Status::internal(e.to_string()))?;

    state.emulator.replace(emulator);

    Ok(TvmEmulatorPrepareResponse { success: true })
}

fn run_get_method(
    state: &mut State,
    req: TvmEmulatorRunGetMethodRequest,
) -> Result<TvmEmulatorRunGetMethodResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu
        .run_get_method(req.method_id, &req.stack_boc)
        .map_err(|e| Status::internal(e.to_string()))?;
    tracing::trace!(method = "run_get_method", "{}", response);

    let response = serde_json::from_str::<TvmResult<TvmEmulatorRunGetMethodResponse>>(&response)
        .map_err(|e| Status::internal(e.to_string()))?;

    response.into()
}

fn send_external_message(
    state: &mut State,
    req: TvmEmulatorSendExternalMessageRequest,
) -> Result<TvmEmulatorSendExternalMessageResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu
        .send_external_message(&req.message_body_boc)
        .map_err(|e| Status::internal(e.to_string()))?;
    tracing::trace!(method = "send_external_message", "{}", response);

    let response =
        serde_json::from_str::<TvmResult<TvmEmulatorSendExternalMessageResponse>>(&response)
            .map_err(|e| Status::internal(e.to_string()))?;

    response.into()
}

fn send_internal_message(
    state: &mut State,
    req: TvmEmulatorSendInternalMessageRequest,
) -> Result<TvmEmulatorSendInternalMessageResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu
        .send_internal_message(&req.message_body_boc, req.amount)
        .map_err(|e| Status::internal(e.to_string()))?;
    tracing::trace!(method = "send_internal_message", "{}", response);

    let response =
        serde_json::from_str::<TvmResult<TvmEmulatorSendInternalMessageResponse>>(&response)
            .map_err(|e| Status::internal(e.to_string()))?;

    response.into()
}

fn set_libraries(
    state: &mut State,
    req: TvmEmulatorSetLibrariesRequest,
) -> Result<TvmEmulatorSetLibrariesResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu
        .set_libraries(&req.libs_boc)
        .map_err(|e| Status::internal(e.to_string()))?;
    tracing::trace!(method = "set_libraries", "{}", response);

    Ok(TvmEmulatorSetLibrariesResponse { success: response })
}

fn set_gas_limit(
    state: &mut State,
    req: TvmEmulatorSetGasLimitRequest,
) -> Result<TvmEmulatorSetGasLimitResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu.set_gas_limit(req.gas_limit);
    tracing::trace!(method = "set_gas_limit", "{}", response);

    Ok(TvmEmulatorSetGasLimitResponse { success: response })
}

#[instrument(skip_all, err)]
fn set_c7(
    state: &mut State,
    req: TvmEmulatorSetC7Request,
) -> Result<TvmEmulatorSetC7Response, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let config = if let Some(cache_key) = &req.config_cache_key {
        if req.config.is_empty() {
            lru_cache()
                .get(cache_key)
                .ok_or(Status::failed_precondition("config cache miss"))?
        } else {
            let config = Arc::new(req.config);
            lru_cache().insert(cache_key.clone(), config.clone());

            config
        }
    } else {
        Arc::new(req.config)
    };

    let response = emu
        .set_c7(
            &req.address,
            req.unixtime,
            req.balance,
            &req.rand_seed_hex,
            &config,
        )
        .map_err(|e| Status::internal(e.to_string()))?;
    tracing::trace!(method = "set_c7", "{}", response);

    Ok(TvmEmulatorSetC7Response { success: response })
}
