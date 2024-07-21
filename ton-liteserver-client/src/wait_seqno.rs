use crate::request::Requestable;
use crate::tl::LiteServerWaitMasterchainSeqno;
use adnl_tcp::serializer::{SerializeBoxed, Serializer};
use std::time::Duration;
use ton_client_util::service::timeout::ToTimeout;

pub struct WaitSeqno<R> {
    prefix: LiteServerWaitMasterchainSeqno,
    request: R,
}

impl<R> WaitSeqno<R>
where
    R: Requestable,
{
    pub fn new(request: R, seqno: i32) -> Self {
        Self::with_timeout(request, seqno, Duration::from_secs(3))
    }

    pub fn with_timeout(request: R, seqno: i32, timeout: Duration) -> Self {
        Self {
            prefix: LiteServerWaitMasterchainSeqno {
                seqno,
                timeout_ms: timeout.as_millis() as i32,
            },
            request,
        }
    }
}

impl<R> SerializeBoxed for WaitSeqno<R>
where
    R: Requestable,
{
    fn serialize_boxed(&self, se: &mut Serializer) {
        self.prefix.serialize_boxed(se);
        self.request.serialize_boxed(se);
    }
}

impl<R> Requestable for WaitSeqno<R>
where
    R: Requestable,
{
    type Response = R::Response;
}

impl<R> ToTimeout for WaitSeqno<R> {
    fn to_timeout(&self) -> Option<Duration> {
        Some(Duration::from_secs(10))
    }
}
