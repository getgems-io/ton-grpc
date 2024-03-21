use tonic::Status;
use uuid::Uuid;

pub type StreamId = Uuid;

pub enum Command<T, R> {
    Request { stream_id: StreamId, request: T, response: tokio::sync::oneshot::Sender<Result<R, Status>> },
    Drop { stream_id: StreamId }
}

#[derive(Debug)]
pub struct Stop<T, R> {
    stream_id: StreamId,
    sender: tokio::sync::mpsc::UnboundedSender<Command<T, R>>
}

impl<T, R> Stop<T, R> {
    pub fn new(stream_id: StreamId, sender: tokio::sync::mpsc::UnboundedSender<Command<T, R>>) -> Self {
        Self { stream_id, sender }
    }
}

impl<T, R> Drop for Stop<T, R> {
    fn drop(&mut self) { let _ = self.sender.send(Command::Drop { stream_id: self.stream_id }); }
}
