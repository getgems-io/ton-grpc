use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use serde_json::Value;
use tower::{Service, ServiceExt};
use crate::session::SessionRequest::{Atomic, RunGetMethod};
use crate::{client::Client, request::Request};
use crate::block::{SmcInfo, SmcLoad, SmcMethodId, SmcRunGetMethod, SmcStack};

#[derive(Clone)]
pub enum SessionRequest {
    RunGetMethod { address: String, method: String, stack: SmcStack },
    Atomic(Request)
}

impl From<Request> for SessionRequest {
    fn from(req: Request) -> Self {
        Atomic(req)
    }
}

pub struct SessionClient {
    client: Client
}

impl SessionClient {
    pub fn new(client: Client) -> Self {
        Self {
            client
        }
    }

    pub fn get_ref(&self) -> &Client {
        &self.client
    }

    pub fn get_mut(&mut self) -> &mut Client {
        &mut self.client
    }
}

impl Service<SessionRequest> for SessionClient {
    type Response = Value;
    type Error = anyhow::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.client.poll_ready(cx)
    }

    fn call(&mut self, req: SessionRequest) -> Self::Future {
        match req {
            Atomic(req) => Box::pin(self.client.call(req)),
            RunGetMethod { address, method, stack} => {
                let mut this = self.client.clone();
                Box::pin(async move {
                    let req = SmcLoad::new(address);
                    let resp = this.ready().await?
                        .call(Request::new(&req)?).await?;

                    let info = serde_json::from_value::<SmcInfo>(resp)?;

                    let req = SmcRunGetMethod::new(
                        info.id,
                        SmcMethodId::Name {name: method},
                        stack
                    );

                    let resp = this.ready().await?
                        .call(Request::new(&req)?).await?;

                    Ok(resp)
                })
            }
        }
    }
}
