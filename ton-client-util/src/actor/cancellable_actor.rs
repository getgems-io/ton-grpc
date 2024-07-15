use crate::actor::Actor;
use tokio::select;
use tokio_util::sync::CancellationToken;

pub struct CancellableActor<T> {
    cancellation_token: CancellationToken,
    inner: T,
}

impl<T> CancellableActor<T> {
    pub fn new(inner: T, cancellation_token: CancellationToken) -> Self {
        Self {
            inner,
            cancellation_token,
        }
    }
}

impl<T> Actor for CancellableActor<T>
where
    T: Actor,
{
    type Output = ();

    async fn run(self) {
        let fut = self.inner.run();

        select! {
            _ = self.cancellation_token.cancelled() => {
                tracing::warn!("actor cancelled");
            },
            _ = fut => {
                tracing::warn!("actor finished");
            },
        };
    }
}
