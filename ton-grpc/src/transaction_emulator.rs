use std::pin::Pin;
use futures::{Stream, StreamExt};
use tonic::{async_trait, Request, Response, Status, Streaming};
use tracing::{error, span};
use anyhow::anyhow;
use tracing::Level;
use async_stream::stream;
use crate::ton::transaction_emulator_service_server::TransactionEmulatorService as BaseTransactionEmulatorService;
use crate::ton::{TransactionEmulatorEmulateRequest, TransactionEmulatorEmulateResponse, TransactionEmulatorPrepareRequest, TransactionEmulatorPrepareResponse, TransactionEmulatorRequest, TransactionEmulatorResponse, TransactionEmulatorSetConfigRequest, TransactionEmulatorSetConfigResponse, TransactionEmulatorSetIgnoreChksigRequest, TransactionEmulatorSetIgnoreChksigResponse, TransactionEmulatorSetLibsRequest, TransactionEmulatorSetLibsResponse, TransactionEmulatorSetLtRequest, TransactionEmulatorSetLtResponse, TransactionEmulatorSetRandSeedRequest, TransactionEmulatorSetRandSeedResponse, TransactionEmulatorSetUnixtimeRequest, TransactionEmulatorSetUnixtimeResponse, TvmResult};
use crate::ton::transaction_emulator_request::Request::*;
use crate::ton::transaction_emulator_response::Response::*;

#[derive(Debug, Default)]
pub struct TransactionEmulatorService;

#[derive(Default)]
struct State {
    emulator: Option<tonlibjson_sys::TransactionEmulator>
}

#[async_trait]
impl BaseTransactionEmulatorService for TransactionEmulatorService {
    type ProcessStream = Pin<Box<dyn Stream<Item=Result<TransactionEmulatorResponse, Status>> + Send>>;

    async fn process(&self, request: Request<Streaming<TransactionEmulatorRequest>>) -> Result<Response<Self::ProcessStream>, Status> {
        let stream = request.into_inner();
        let mut state = State::default();

        let output = stream! {
            for await msg in stream {
                match msg {
                    Ok(TransactionEmulatorRequest { request_id, request: Some(req) }) => {
                        let span = span!(Level::TRACE, "transaction emulator request", request_id=request_id, request = ?req);
                        let _guard = span.enter();

                        let response = match req {
                            Prepare(req) => prepare(&mut state, req).map(PrepareResponse),
                            _ => {
                                if let Some(emu) = state.emulator.as_ref() {
                                    match req {
                                        Emulate(req) => emulate(emu, req).map(EmulateResponse),
                                        SetUnixtime(req) => set_unixtime(emu, req).map(SetUnixtimeResponse),
                                        SetLt(req) => set_lt(emu, req).map(SetLtResponse),
                                        SetRandSeed(req) => set_rand_seed(emu, req).map(SetRandSeedResponse),
                                        SetIgnoreChksig(req) => set_ignore_chksig(emu, req).map(SetIgnoreChksigResponse),
                                        SetConfig(req) => set_config(emu, req).map(SetConfigResponse),
                                        SetLibs(req) => set_libs(emu, req).map(SetLibsResponse),
                                        Prepare(_) => unreachable!()
                                    }
                                } else {
                                    Err(anyhow!("emulator not initialized"))
                                }
                            }
                        };

                        yield response
                            .map(|r| TransactionEmulatorResponse { request_id, response: Some(r) })
                            .map_err(|e| {
                                error!(error = ?e);

                                Status::internal(e.to_string())
                            })
                    },
                    Ok(TransactionEmulatorRequest { request_id, request: None }) => {
                        error!(error = ?anyhow!("empty request"), request_id=request_id);

                        break
                    },
                    Err(e) =>  {
                        error!(error = ?e);

                        break
                    }
                }
            }
        }.boxed();

        Ok(Response::new(output))
    }
}

fn prepare(state: &mut State, req: TransactionEmulatorPrepareRequest) -> anyhow::Result<TransactionEmulatorPrepareResponse> {
    let emulator = tonlibjson_sys::TransactionEmulator::new(&req.config_boc, req.vm_log_level)?;

    let _ = state.emulator.replace(emulator);

    Ok(TransactionEmulatorPrepareResponse { success: true })
}

fn emulate(emu: &tonlibjson_sys::TransactionEmulator, req: TransactionEmulatorEmulateRequest) -> anyhow::Result<TransactionEmulatorEmulateResponse> {
    let response = emu.emulate(&req.shard_account_boc, &req.message_boc)?;
    tracing::trace!(method="emulate", "{}", response);

    let response = serde_json::from_str::<TvmResult<TransactionEmulatorEmulateResponse>>(response)?;

    response.into()
}

fn set_unixtime(emu: &tonlibjson_sys::TransactionEmulator, req: TransactionEmulatorSetUnixtimeRequest) -> anyhow::Result<TransactionEmulatorSetUnixtimeResponse> {
    let response = emu.set_unixtime(req.unixtime);
    tracing::trace!(method="set_unixtime", "{}", response);

    Ok(TransactionEmulatorSetUnixtimeResponse { success: true })
}

fn set_lt(emu: &tonlibjson_sys::TransactionEmulator, req: TransactionEmulatorSetLtRequest) -> anyhow::Result<TransactionEmulatorSetLtResponse> {
    let response = emu.set_lt(req.lt);
    tracing::trace!(method="set_lt", "{}", response);

    Ok(TransactionEmulatorSetLtResponse { success: true })
}

fn set_rand_seed(emu: &tonlibjson_sys::TransactionEmulator, req: TransactionEmulatorSetRandSeedRequest) -> anyhow::Result<TransactionEmulatorSetRandSeedResponse> {
    let response = emu.set_rand_seed(&req.rand_seed)?;
    tracing::trace!(method="set_rand_seed", "{}", response);

    Ok(TransactionEmulatorSetRandSeedResponse { success: true })
}

fn set_ignore_chksig(emu: &tonlibjson_sys::TransactionEmulator, req: TransactionEmulatorSetIgnoreChksigRequest) -> anyhow::Result<TransactionEmulatorSetIgnoreChksigResponse> {
    let response = emu.set_ignore_chksig(req.ignore_chksig);
    tracing::trace!(method="set_ignore_chksig", "{}", response);

    Ok(TransactionEmulatorSetIgnoreChksigResponse { success: true })
}

fn set_config(emu: &tonlibjson_sys::TransactionEmulator, req: TransactionEmulatorSetConfigRequest) -> anyhow::Result<TransactionEmulatorSetConfigResponse> {
    let response = emu.set_config(&req.config)?;
    tracing::trace!(method="set_config", "{}", response);

    Ok(TransactionEmulatorSetConfigResponse { success: true })
}

fn set_libs(emu: &tonlibjson_sys::TransactionEmulator, req: TransactionEmulatorSetLibsRequest) -> anyhow::Result<TransactionEmulatorSetLibsResponse> {
    let response = emu.set_libs(&req.libs)?;
    tracing::trace!(method="set_libs", "{}", response);

    Ok(TransactionEmulatorSetLibsResponse { success: true })
}
