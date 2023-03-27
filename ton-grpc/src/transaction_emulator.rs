use std::future::ready;
use std::pin::Pin;
use futures::{Stream, StreamExt};
use tonic::{async_trait, Request, Response, Status, Streaming};
use tracing::{error};
use anyhow::anyhow;
use crate::ton::transaction_emulator_server::TransactionEmulator;
use crate::ton::{TransactionEmulatorEmulateRequest, TransactionEmulatorEmulateResponse, TransactionEmulatorPrepareRequest, TransactionEmulatorPrepareResponse, TransactionEmulatorRequest, TransactionEmulatorResponse, TvmResult};
use crate::ton::transaction_emulator_request::Request::{Prepare, Emulate};
use crate::ton::transaction_emulator_response::Response::{PrepareResponse, EmulateResponse};

#[derive(Debug, Default)]
pub struct TransactionEmulatorService;

#[derive(Default)]
struct State {
    emulator: Option<tonlibjson_sys::TransactionEmulator>
}

#[async_trait]
impl TransactionEmulator for TransactionEmulatorService {
    type ProcessStream = Pin<Box<dyn Stream<Item=Result<TransactionEmulatorResponse, Status>> + Send>>;

    async fn process(&self, request: Request<Streaming<TransactionEmulatorRequest>>) -> Result<Response<Self::ProcessStream>, Status> {
        let stream = request.into_inner();

        let output = stream.scan(State::default(), |state, msg| {
            match msg {
                Ok(TransactionEmulatorRequest { request: Some(req) }) => {
                    let response = match req {
                        Prepare(req) => prepare(state, req).map(PrepareResponse),
                        Emulate(req) => emulate(state, req).map(EmulateResponse)
                    };

                    ready(Some(response
                        .map(|r| TransactionEmulatorResponse { response: Some(r) })
                        .map_err(|e| Status::internal(e.to_string()))))
                },
                Ok(TransactionEmulatorRequest { request: None }) => {
                    error!(error = ?anyhow!("empty request"));

                    ready(None)
                },
                Err(e) =>  {
                    error!(error = ?e);

                    ready(None)
                }
            }
        }).boxed();

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

    let response = serde_json::from_str::<TvmResult<TransactionEmulatorEmulateResponse>>(response)?;

    response.into()
}
