use crate::ton::message_service_server::MessageService as BaseMessageService;
use crate::ton::{SendRequest, SendResponse};
use derive_new::new;
use ton_client::{Client, TonService};
use tonic::{Request, Response, Status, async_trait};

#[derive(new)]
pub struct MessageService<S: TonService> {
    client: Client<S>,
}

#[async_trait]
impl<S: TonService> BaseMessageService for MessageService<S> {
    #[tracing::instrument(skip_all, err)]
    async fn send_message(
        &self,
        request: Request<SendRequest>,
    ) -> Result<Response<SendResponse>, Status> {
        let msg = request.into_inner();

        let mut client = self.client.clone();
        let hash = client
            .send_message_returning_hash(&msg.body)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(SendResponse { hash }))
    }
}

#[cfg(test)]
mod integration {
    use crate::message::MessageService;
    use crate::ton::SendRequest;
    use crate::ton::message_service_client::MessageServiceClient;
    use crate::ton::message_service_server::MessageServiceServer;
    use testcontainers_ton::{LocalLiteServer, SharedLiteServer};
    use tokio::net::TcpListener;
    use ton_client::TonClientBuilder;
    use tonic::transport::Channel;
    use tonlibjson_client::MakeTonlibjsonAdapter;

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

    async fn setup() -> (SharedLiteServer, MessageServiceClient<Channel>) {
        let server = LocalLiteServer::shared().await.unwrap();
        let mut client = TonClientBuilder::<MakeTonlibjsonAdapter>::from_config(server.config())
            .build()
            .unwrap();
        client.wait_ready().await.unwrap();

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
