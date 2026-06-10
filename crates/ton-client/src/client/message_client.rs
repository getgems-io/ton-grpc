use crate::Client;
use crate::RequestHandler;
use ton_tower::request::{SendMessage, SendMessageReturningHash};
use tower::ServiceExt;

impl<S> Client<S>
where
    S: RequestHandler<SendMessage>,
{
    pub async fn send_message<M>(&mut self, message: M) -> anyhow::Result<()>
    where
        M: ToString,
    {
        let body = message.to_string();
        self.oneshot(SendMessage { body }).await
    }
}

impl<S> Client<S>
where
    S: RequestHandler<SendMessageReturningHash>,
{
    pub async fn send_message_returning_hash<M>(&mut self, message: M) -> anyhow::Result<String>
    where
        M: ToString,
    {
        let body = message.to_string();
        self.oneshot(SendMessageReturningHash { body }).await
    }
}
