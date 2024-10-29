#![allow(clippy::blocks_in_conditions)]

use crate::ton::message_service_server::MessageService as BaseMessageService;
use crate::ton::{SendRequest, SendResponse};
use derive_new::new;
use tonic::{async_trait, Request, Response, Status};
use tonlibjson_client::ton::TonClient;

#[derive(new)]
pub struct MessageService {
    client: TonClient,
}

#[async_trait]
impl BaseMessageService for MessageService {
    #[tracing::instrument(skip_all, err)]
    async fn send_message(
        &self,
        request: Request<SendRequest>,
    ) -> Result<Response<SendResponse>, Status> {
        let msg = request.into_inner();

        let hash = self
            .client
            .send_message_returning_hash(&msg.body)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(SendResponse { hash }))
    }
}
