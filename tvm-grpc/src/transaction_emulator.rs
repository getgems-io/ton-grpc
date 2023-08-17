use std::pin::Pin;
use futures::{Stream, StreamExt};
use tonic::{async_trait, Request, Response, Status, Streaming};
use tracing::{error};
use anyhow::anyhow;
use async_stream::stream;
use crate::threaded::{Command, Stop};
use crate::tvm::transaction_emulator_service_server::TransactionEmulatorService as BaseTransactionEmulatorService;
use crate::tvm::{transaction_emulator_request, transaction_emulator_response, TransactionEmulatorEmulateRequest, TransactionEmulatorEmulateResponse, TransactionEmulatorPrepareRequest, TransactionEmulatorPrepareResponse, TransactionEmulatorRequest, TransactionEmulatorResponse, TransactionEmulatorSetConfigRequest, TransactionEmulatorSetConfigResponse, TransactionEmulatorSetIgnoreChksigRequest, TransactionEmulatorSetIgnoreChksigResponse, TransactionEmulatorSetLibsRequest, TransactionEmulatorSetLibsResponse, TransactionEmulatorSetLtRequest, TransactionEmulatorSetLtResponse, TransactionEmulatorSetRandSeedRequest, TransactionEmulatorSetRandSeedResponse, TransactionEmulatorSetUnixtimeRequest, TransactionEmulatorSetUnixtimeResponse, TvmResult};
use crate::tvm::transaction_emulator_request::Request::*;
use crate::tvm::transaction_emulator_response::Response::*;

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

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Command<transaction_emulator_request::Request, transaction_emulator_response::Response>>();
        let stop = Stop::new(tx.clone());

        rayon::spawn(move || {
            let mut state = State::default();

            while let Some(command) = rx.blocking_recv() {
                match command {
                    Command::Request { request, response: oneshot } => {
                        let response = match request {
                            Prepare(req) => prepare(&mut state, req).map(PrepareResponse),
                            Emulate(req) => emulate(&mut state, req).map(EmulateResponse),
                            SetUnixtime(req) => set_unixtime(&mut state, req).map(SetUnixtimeResponse),
                            SetLt(req) => set_lt(&mut state, req).map(SetLtResponse),
                            SetRandSeed(req) => set_rand_seed(&mut state, req).map(SetRandSeedResponse),
                            SetIgnoreChksig(req) => set_ignore_chksig(&mut state, req).map(SetIgnoreChksigResponse),
                            SetConfig(req) => set_config(&mut state, req).map(SetConfigResponse),
                            SetLibs(req) => set_libs(&mut state, req).map(SetLibsResponse),
                        };

                        oneshot.send(response).expect("failed to send response");
                    }
                    Command::Drop => { break; }
                }
            }
        });

        let output = stream! {
            for await msg in stream {
                match msg {
                    Ok(TransactionEmulatorRequest { request_id, request: Some(req) }) => {
                        let (to, ro) = tokio::sync::oneshot::channel();

                        let _ = tx.send(Command::Request { request: req, response: to });
                        let response = ro.await.expect("failed to receive response");

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

            drop(stop);
        }.boxed();

        Ok(Response::new(output))
    }
}

fn prepare(state: &mut State, req: TransactionEmulatorPrepareRequest) -> anyhow::Result<TransactionEmulatorPrepareResponse> {
    let emulator = tonlibjson_sys::TransactionEmulator::new(&req.config_boc, req.vm_log_level)?;

    let _ = state.emulator.replace(emulator);

    Ok(TransactionEmulatorPrepareResponse { success: true })
}

fn emulate(state: &mut State, req: TransactionEmulatorEmulateRequest) -> anyhow::Result<TransactionEmulatorEmulateResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.emulate(&req.shard_account_boc, &req.message_boc)?;
    tracing::trace!(method="emulate", "{}", response);

    let response = serde_json::from_str::<TvmResult<TransactionEmulatorEmulateResponse>>(&response)?;

    response.into()
}

fn set_unixtime(state: &mut State, req: TransactionEmulatorSetUnixtimeRequest) -> anyhow::Result<TransactionEmulatorSetUnixtimeResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.set_unixtime(req.unixtime);
    tracing::trace!(method="set_unixtime", "{}", response);

    Ok(TransactionEmulatorSetUnixtimeResponse { success: true })
}

fn set_lt(state: &mut State, req: TransactionEmulatorSetLtRequest) -> anyhow::Result<TransactionEmulatorSetLtResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.set_lt(req.lt);
    tracing::trace!(method="set_lt", "{}", response);

    Ok(TransactionEmulatorSetLtResponse { success: true })
}

fn set_rand_seed(state: &mut State, req: TransactionEmulatorSetRandSeedRequest) -> anyhow::Result<TransactionEmulatorSetRandSeedResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.set_rand_seed(&req.rand_seed)?;
    tracing::trace!(method="set_rand_seed", "{}", response);

    Ok(TransactionEmulatorSetRandSeedResponse { success: true })
}

fn set_ignore_chksig(state: &mut State, req: TransactionEmulatorSetIgnoreChksigRequest) -> anyhow::Result<TransactionEmulatorSetIgnoreChksigResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.set_ignore_chksig(req.ignore_chksig);
    tracing::trace!(method="set_ignore_chksig", "{}", response);

    Ok(TransactionEmulatorSetIgnoreChksigResponse { success: true })
}

fn set_config(state: &mut State, req: TransactionEmulatorSetConfigRequest) -> anyhow::Result<TransactionEmulatorSetConfigResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.set_config(&req.config)?;
    tracing::trace!(method="set_config", "{}", response);

    Ok(TransactionEmulatorSetConfigResponse { success: true })
}

fn set_libs(state: &mut State, req: TransactionEmulatorSetLibsRequest) -> anyhow::Result<TransactionEmulatorSetLibsResponse> {
    let Some(emu) = state.emulator.as_ref() else {
        return Err(anyhow!("emulator not initialized"));
    };

    let response = emu.set_libs(&req.libs)?;
    tracing::trace!(method="set_libs", "{}", response);

    Ok(TransactionEmulatorSetLibsResponse { success: true })
}
