use pin_project::pin_project;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::Service;
use tower::make::MakeService;
use tracing::trace;

pub struct Reconnect<M, Target>
where
    M: Service<Target>,
{
    mk_service: M,
    state: State<M::Future, M::Response>,
    target: Target,
    error: Option<M::Error>,
}

#[derive(Debug)]
enum State<F, S> {
    Idle,
    Connecting(F),
    Connected(S),
}

impl<M, Target> Reconnect<M, Target>
where
    M: Service<Target>,
{
    pub const fn new(mk_service: M, target: Target) -> Self {
        Reconnect {
            mk_service,
            state: State::Idle,
            target,
            error: None,
        }
    }
}

impl<M, Target, S, Request> Service<Request> for Reconnect<M, Target>
where
    M: Service<Target, Response = S>,
    S: Service<Request>,
    M::Future: Unpin,
    anyhow::Error: From<M::Error> + From<S::Error>,
    Target: Clone,
{
    type Response = S::Response;
    type Error = anyhow::Error;
    type Future = ResponseFuture<S::Future, M::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            match &mut self.state {
                State::Idle => {
                    trace!("poll_ready; idle");
                    match self.mk_service.poll_ready(cx) {
                        Poll::Ready(r) => r?,
                        Poll::Pending => {
                            trace!("poll_ready; MakeService not ready");
                            return Poll::Pending;
                        }
                    }

                    let fut = self.mk_service.make_service(self.target.clone());
                    self.state = State::Connecting(fut);
                    continue;
                }
                State::Connecting(f) => {
                    trace!("poll_ready; connecting");
                    match Pin::new(f).poll(cx) {
                        Poll::Ready(Ok(service)) => {
                            self.state = State::Connected(service);
                        }
                        Poll::Pending => {
                            trace!("poll_ready; not ready");
                            return Poll::Pending;
                        }
                        Poll::Ready(Err(e)) => {
                            trace!("poll_ready; error");
                            self.state = State::Idle;
                            self.error = Some(e);
                            break;
                        }
                    }
                }
                State::Connected(inner) => {
                    trace!("poll_ready; connected");
                    match inner.poll_ready(cx) {
                        Poll::Ready(Ok(())) => {
                            trace!("poll_ready; ready");
                            return Poll::Ready(Ok(()));
                        }
                        Poll::Pending => {
                            trace!("poll_ready; not ready");
                            return Poll::Pending;
                        }
                        Poll::Ready(Err(_)) => {
                            trace!("poll_ready; error");
                            self.state = State::Idle;
                        }
                    }
                }
            }
        }

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        if let Some(error) = self.error.take() {
            return ResponseFuture::error(error);
        }

        let service = match self.state {
            State::Connected(ref mut service) => service,
            _ => panic!("service not ready; poll_ready must be called first"),
        };

        let fut = service.call(request);
        ResponseFuture::new(fut)
    }
}

impl<M, Target> fmt::Debug for Reconnect<M, Target>
where
    M: Service<Target> + fmt::Debug,
    M::Future: fmt::Debug,
    M::Response: fmt::Debug,
    Target: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Reconnect")
            .field("mk_service", &self.mk_service)
            .field("state", &self.state)
            .field("target", &self.target)
            .finish()
    }
}

#[pin_project]
pub struct ResponseFuture<F, E> {
    #[pin]
    inner: Inner<F, E>,
}

#[pin_project(project = InnerProj)]
enum Inner<F, E> {
    Future {
        #[pin]
        fut: F,
    },
    Error {
        error: Option<E>,
    },
}

impl<F, E> ResponseFuture<F, E> {
    pub(crate) fn new(fut: F) -> Self {
        ResponseFuture {
            inner: Inner::Future { fut },
        }
    }

    pub(crate) fn error(error: E) -> Self {
        ResponseFuture {
            inner: Inner::Error { error: Some(error) },
        }
    }
}

impl<F, T, E, ME> Future for ResponseFuture<F, ME>
where
    F: Future<Output = Result<T, E>>,
    E: Into<anyhow::Error>,
    ME: Into<anyhow::Error>,
{
    type Output = Result<T, anyhow::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        match me.inner.project() {
            InnerProj::Future { fut } => fut.poll(cx).map_err(Into::into),
            InnerProj::Error { error } => {
                let e = error.take().expect("polled after ready").into();
                Poll::Ready(Err(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::Sequence;
    use mockall::mock;
    use std::future::{Ready, ready};
    use tower::ServiceExt;

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn connects_and_serves_request() {
        let svc = Reconnect::new(maker_returning(vec![service_ready()]), "target".to_string());

        let result = svc.oneshot(Request).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn returns_error_when_make_service_fails() {
        let maker = maker_failing("failed to create svc");
        let mut reconnect = Reconnect::new(maker, "target".to_string());

        let result = (&mut reconnect).oneshot(Request).await;

        assert_eq!(result.err().unwrap().to_string(), "failed to create svc");
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn skips_failing_svc() {
        let maker = maker_returning(vec![service_failing("fail"), service_ready()]);
        let mut reconnect = Reconnect::new(maker, "target".to_string());

        let first = (&mut reconnect).oneshot(Request).await;

        assert!(first.is_ok());
    }

    #[derive(Debug)]
    struct Request;

    mock! {
        Service {}

        impl Service<Request> for Service {
            type Response = ();
            type Error = anyhow::Error;
            type Future = Ready<Result<(), anyhow::Error>>;

            fn poll_ready<'a>(&mut self, _cx: &mut Context<'a>) -> Poll<Result<(), anyhow::Error>>;
            fn call(&mut self, _req: Request) -> Ready<Result<(), anyhow::Error>>;
        }
    }

    mock! {
        Maker {}

        impl Service<String> for Maker {
            type Response = MockService;
            type Error = anyhow::Error;
            type Future = Ready<Result<MockService, anyhow::Error>>;

            fn poll_ready<'a>(&mut self, _cx: &mut Context<'a>) -> Poll<Result<(), anyhow::Error>>;
            fn call(&mut self, _target: String) -> Ready<Result<MockService, anyhow::Error>>;
        }
    }

    fn service_ready() -> MockService {
        let mut svc = MockService::new();
        svc.expect_poll_ready().returning(|_| Poll::Ready(Ok(())));
        svc.expect_call().returning(|_| ready(Ok(())));
        svc
    }

    fn service_failing(msg: &str) -> MockService {
        let mut svc = MockService::new();
        let msg = msg.to_string();
        svc.expect_poll_ready()
            .returning(move |_| Poll::Ready(Err(anyhow::anyhow!(msg.clone()))));
        svc.expect_call().never();
        svc
    }

    fn maker_ready() -> MockMaker {
        let mut maker = MockMaker::new();
        maker.expect_poll_ready().returning(|_| Poll::Ready(Ok(())));
        maker
    }

    fn maker_returning(svc: impl IntoIterator<Item = MockService>) -> MockMaker {
        let mut maker = maker_ready();
        let mut seq = Sequence::new();
        for svc in svc {
            maker
                .expect_call()
                .once()
                .in_sequence(&mut seq)
                .return_once(move |_| ready(Ok(svc)));
        }
        maker
    }

    fn maker_failing(msg: &str) -> MockMaker {
        let mut maker = maker_ready();
        let msg = msg.to_string();
        maker
            .expect_call()
            .returning(move |_| ready(Err(anyhow::anyhow!(msg.clone()))));
        maker
    }
}
