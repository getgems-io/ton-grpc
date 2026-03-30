#![allow(clippy::blocks_in_conditions)]

use crate::ton::message_service_server::MessageService as BaseMessageService;
use crate::ton::{SendRequest, SendResponse};
use derive_new::new;
use tonic::{Request, Response, Status, async_trait};
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

#[cfg(test)]
mod integration {
    use crate::message::MessageService;
    use crate::ton::message_service_client::MessageServiceClient;
    use crate::ton::message_service_server::MessageServiceServer;
    use crate::ton::SendRequest;
    use testcontainers_ton::LocalLiteServer;
    use tokio::net::TcpListener;
    use tonlibjson_client::ton::TonClientBuilder;
    use tonic::transport::Channel;

    // TODO[akostylev0]: add test for success send message
    #[tokio::test]
    async fn should_fail_send_invalid_message() {
        let (_server, mut client) = setup().await;

        let result = client
            .send_message(SendRequest {
                body: "invalid_boc".to_string(),
            })
            .await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
    }

    async fn setup() -> (LocalLiteServer, MessageServiceClient<Channel>) {
        let server = LocalLiteServer::new().await.unwrap();
        let mut client = TonClientBuilder::from_config(server.config())
            .build()
            .unwrap();
        client.ready().await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(MessageServiceServer::new(MessageService::new(client)))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        let channel = Channel::from_shared(format!("http://{}", addr))
            .unwrap()
            .connect()
            .await
            .unwrap();

        (server, MessageServiceClient::new(channel))
    }
}
