use std::any::TypeId;
use std::future;
use std::sync::Arc;
use tower::retry::budget::Budget;
use tower::retry::Policy;
use crate::block::{RawSendMessage, RawSendMessageReturnHash};

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

impl<T: Clone + 'static, Res, E> Policy<T, Res, E> for RetryPolicy {
    type Future = future::Ready<Self>;

    fn retry(&self, _: &T, result: Result<&Res, &E>) -> Option<Self::Future> {
        match result {
            Ok(_) => {
                self.budget.deposit();

                None
            }
            Err(_) => {
                // TODO[akostylev0] rewrite to trait
                let type_of = TypeId::of::<T>();
                if type_of == TypeId::of::<RawSendMessageReturnHash>() {
                    None
                } else if type_of == TypeId::of::<RawSendMessage>() {
                    None
                } else {
                    match self.budget.withdraw() {
                        Ok(_) => Some(future::ready(self.clone())),
                        Err(_) => None
                    }
                }
            }
        }
    }

    fn clone_request(&self, req: &T) -> Option<T> {
        Some(req.clone())
    }
}
