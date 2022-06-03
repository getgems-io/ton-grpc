use std::sync::Arc;
use futures::future;
use tower::retry::budget::Budget;
use tower::retry::Policy;

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

impl<E, Req, Res> Policy<Req, Res, E> for RetryPolicy where Req : Clone {
    type Future = future::Ready<Self>;

    fn retry(&self, _: &Req, result: Result<&Res, &E>) -> Option<Self::Future> {
        match result {
            Ok(_) => {
                self.budget.deposit();

                None
            }
            Err(_) => {
                match self.budget.withdraw() {
                    Ok(_) => Some(future::ready(self.clone())),
                    Err(_) => None
                }
            }
        }
    }

    fn clone_request(&self, req: &Req) -> Option<Req> {
        Some(req.clone())
    }
}
