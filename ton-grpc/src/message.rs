use tonic::{Request, Response, Status};
use tonlibjson_client::ton::TonClient;
use crate::ton::message_service_server::MessageService as BaseMessageService;
use crate::ton::{SendRequest, SendResponse};

pub struct MessageService {
    client: TonClient
}

impl BaseMessageService for MessageService {
    async fn send_message(&self, request: Request<SendRequest>) -> Result<Response<SendResponse>, Status> {
        let msg = request.into_inner();

        let response = self.client.send_message(&msg.body).await?;
    }
}
