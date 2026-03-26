#![allow(clippy::blocks_in_conditions)]

use crate::ton::message_service_server::MessageService as BaseMessageService;
use crate::ton::{SendRequest, SendResponse};
use derive_new::new;
use ton_client::client::TonClient;
use tonic::{Request, Response, Status, async_trait};

#[derive(new)]
pub struct MessageService<T> {
    client: T,
}

#[async_trait]
impl<T: TonClient> BaseMessageService for MessageService<T> {
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
