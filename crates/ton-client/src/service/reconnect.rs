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
