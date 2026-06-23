pub mod actor;
pub mod request;
pub mod response;
pub mod service;

pub trait Request {
    type Response;
}

pub trait IntoRequest {
    type Request: Request;

    fn into_request(self) -> Self::Request;
}

impl<R> IntoRequest for R
where
    R: Request,
{
    type Request = Self;

    fn into_request(self) -> R {
        self
    }
}
