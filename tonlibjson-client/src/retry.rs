use std::sync::Arc;
use futures::future;
use tower::retry::budget::Budget;
use tower::retry::Policy;
use crate::balance::BalanceRequest;
use crate::request::{Request, RequestBody};
use crate::session::SessionRequest;

#[derive(Clone)]
pub struct RetryPolicy {
    budget: Arc<Budget>
}

impl RetryPolicy {
    pub fn new(budget: Budget) -> Self {
        Self {
            budget: Arc::new(budget)
        }
    }
}

impl<E, Res> Policy<BalanceRequest, Res, E> for RetryPolicy {
    type Future = future::Ready<Self>;

    fn retry(&self, req: &BalanceRequest, result: Result<&Res, &E>) -> Option<Self::Future> {
        match result {
            Ok(_) => {
                self.budget.deposit();

                None
            }
            Err(_) => {
                match req.request {
                    SessionRequest::Atomic(Request { body: RequestBody::RawSendMessageReturnHash(_), .. }) => None,
                    SessionRequest::Atomic(Request { body: RequestBody::RawSendMessage(_), .. }) => None,
                    _ => match self.budget.withdraw() {
                        Ok(_) => Some(future::ready(self.clone())),
                        Err(_) => None
                    }
                }
            }
        }
    }

    fn clone_request(&self, req: &BalanceRequest) -> Option<BalanceRequest> {
        let inner = match req.request {
            SessionRequest::Atomic(ref req) => SessionRequest::new_atomic(req.with_new_id()),
            _ => req.request.clone()
        };

        Some(BalanceRequest::new(req.route, inner))
    }
}
