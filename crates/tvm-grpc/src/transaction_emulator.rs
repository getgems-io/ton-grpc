use crate::threaded::{Command, Stop, StreamId};
use crate::tvm::transaction_emulator_request::Request::*;
use crate::tvm::transaction_emulator_response::Response::*;
use crate::tvm::transaction_emulator_service_server::TransactionEmulatorService as BaseTransactionEmulatorService;
use crate::tvm::{
    transaction_emulator_request, transaction_emulator_response, TransactionEmulatorEmulateRequest,
    TransactionEmulatorEmulateResponse, TransactionEmulatorPrepareRequest,
    TransactionEmulatorPrepareResponse, TransactionEmulatorRequest, TransactionEmulatorResponse,
    TransactionEmulatorSetConfigRequest, TransactionEmulatorSetConfigResponse,
    TransactionEmulatorSetIgnoreChksigRequest, TransactionEmulatorSetIgnoreChksigResponse,
    TransactionEmulatorSetLibsRequest, TransactionEmulatorSetLibsResponse,
    TransactionEmulatorSetLtRequest, TransactionEmulatorSetLtResponse,
    TransactionEmulatorSetRandSeedRequest, TransactionEmulatorSetRandSeedResponse,
    TransactionEmulatorSetUnixtimeRequest, TransactionEmulatorSetUnixtimeResponse, TvmResult,
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
use tracing::instrument;

#[derive(Debug)]
pub struct TransactionEmulatorService {
    tx: UnboundedSender<
        Command<transaction_emulator_request::Request, transaction_emulator_response::Response>,
    >,
}

#[derive(Default)]
struct State {
    emulator: Option<tonlibjson_sys::TransactionEmulator>,
}

fn lru_cache() -> &'static Cache<String, Arc<String>> {
    static LRU_CACHE: OnceLock<Cache<String, Arc<String>>> = OnceLock::new();

    LRU_CACHE.get_or_init(|| Cache::new(32))
}

impl Default for TransactionEmulatorService {
    fn default() -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<
            Command<transaction_emulator_request::Request, transaction_emulator_response::Response>,
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
                            Prepare(req) => prepare(state, req).map(PrepareResponse),
                            Emulate(req) => emulate(state, req).map(EmulateResponse),
                            SetUnixtime(req) => set_unixtime(state, req).map(SetUnixtimeResponse),
                            SetLt(req) => set_lt(state, req).map(SetLtResponse),
                            SetRandSeed(req) => set_rand_seed(state, req).map(SetRandSeedResponse),
                            SetIgnoreChksig(req) => {
                                set_ignore_chksig(state, req).map(SetIgnoreChksigResponse)
                            }
                            SetConfig(req) => set_config(state, req).map(SetConfigResponse),
                            SetLibs(req) => set_libs(state, req).map(SetLibsResponse),
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

#[async_trait]
impl BaseTransactionEmulatorService for TransactionEmulatorService {
    type ProcessStream =
        Pin<Box<dyn Stream<Item = Result<TransactionEmulatorResponse, Status>> + Send>>;

    async fn process(
        &self,
        request: Request<Streaming<TransactionEmulatorRequest>>,
    ) -> Result<Response<Self::ProcessStream>, Status> {
        let stream_id = StreamId::new_v4();
        let stream = request.into_inner();
        let stream = stream.timeout(Duration::from_secs(3));
        let stop = Stop::new(stream_id, self.tx.clone());
        let tx = self.tx.clone();

        let output = stream! {
            for await msg in stream {
                match msg {
                    Ok(Ok(TransactionEmulatorRequest { request_id, request: Some(req) })) => {
                        let (to, ro) = tokio::sync::oneshot::channel();

                        let _ = tx.send(Command::Request { stream_id, request: req, response: to });
                        let Ok(response) = ro.await else {
                            break
                        };

                        yield response.map(|r| TransactionEmulatorResponse { request_id, response: Some(r) })
                    },
                    Ok(Ok(TransactionEmulatorRequest { request_id, request: None })) => {
                        tracing::error!(error = ?anyhow!("empty request"), request_id=request_id);

                        break
                    },
                    Ok(Err(e)) => {
                        tracing::error!(error = ?e);

                        break
                    }
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
}

#[instrument(skip_all, err)]
fn prepare(
    state: &mut State,
    req: TransactionEmulatorPrepareRequest,
) -> Result<TransactionEmulatorPrepareResponse, Status> {
    let config = if let Some(cache_key) = &req.config_cache_key {
        if req.config_boc.is_empty() {
            lru_cache()
                .get(cache_key)
                .ok_or(Status::failed_precondition("config cache miss"))?
        } else {
            let config = Arc::new(req.config_boc.clone());
            lru_cache().insert(cache_key.clone(), config.clone());

            config
        }
    } else {
        Arc::new(req.config_boc)
    };

    let emulator = tonlibjson_sys::TransactionEmulator::new(&config, req.vm_log_level)
        .map_err(|e| Status::internal(e.to_string()))?;

    let _ = state.emulator.replace(emulator);

    Ok(TransactionEmulatorPrepareResponse { success: true })
}

fn emulate(
    state: &mut State,
    req: TransactionEmulatorEmulateRequest,
) -> Result<TransactionEmulatorEmulateResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu
        .emulate(&req.shard_account_boc, &req.message_boc)
        .map_err(|e| Status::internal(e.to_string()))?;

    tracing::trace!(method = "emulate", "{}", response);

    let response = serde_json::from_str::<TvmResult<TransactionEmulatorEmulateResponse>>(&response)
        .map_err(|e| Status::internal(e.to_string()))?;

    response.into()
}

fn set_unixtime(
    state: &mut State,
    req: TransactionEmulatorSetUnixtimeRequest,
) -> Result<TransactionEmulatorSetUnixtimeResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu.set_unixtime(req.unixtime);
    tracing::trace!(method = "set_unixtime", "{}", response);

    Ok(TransactionEmulatorSetUnixtimeResponse { success: true })
}

fn set_lt(
    state: &mut State,
    req: TransactionEmulatorSetLtRequest,
) -> Result<TransactionEmulatorSetLtResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu.set_lt(req.lt);
    tracing::trace!(method = "set_lt", "{}", response);

    Ok(TransactionEmulatorSetLtResponse { success: true })
}

fn set_rand_seed(
    state: &mut State,
    req: TransactionEmulatorSetRandSeedRequest,
) -> Result<TransactionEmulatorSetRandSeedResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu
        .set_rand_seed(&req.rand_seed)
        .map_err(|e| Status::internal(e.to_string()))?;
    tracing::trace!(method = "set_rand_seed", "{}", response);

    Ok(TransactionEmulatorSetRandSeedResponse { success: true })
}

fn set_ignore_chksig(
    state: &mut State,
    req: TransactionEmulatorSetIgnoreChksigRequest,
) -> Result<TransactionEmulatorSetIgnoreChksigResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu.set_ignore_chksig(req.ignore_chksig);
    tracing::trace!(method = "set_ignore_chksig", "{}", response);

    Ok(TransactionEmulatorSetIgnoreChksigResponse { success: true })
}

fn set_config(
    state: &mut State,
    req: TransactionEmulatorSetConfigRequest,
) -> Result<TransactionEmulatorSetConfigResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu
        .set_config(&req.config)
        .map_err(|e| Status::internal(e.to_string()))?;
    tracing::trace!(method = "set_config", "{}", response);

    Ok(TransactionEmulatorSetConfigResponse { success: true })
}

fn set_libs(
    state: &mut State,
    req: TransactionEmulatorSetLibsRequest,
) -> Result<TransactionEmulatorSetLibsResponse, Status> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(Status::internal("emulator not initialized"));
    };

    let response = emu
        .set_libs(&req.libs)
        .map_err(|e| Status::internal(e.to_string()))?;
    tracing::trace!(method = "set_libs", "{}", response);

    Ok(TransactionEmulatorSetLibsResponse { success: true })
}
