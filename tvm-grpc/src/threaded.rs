pub enum Command<T, R> {
    Request { request: T, response: tokio::sync::oneshot::Sender<anyhow::Result<R>> },
    Drop
}

#[derive(Debug)]
pub struct Stop<T, R> { sender: tokio::sync::mpsc::UnboundedSender<Command<T, R>> }

impl<T, R> Stop<T, R> {
    pub fn new(sender: tokio::sync::mpsc::UnboundedSender<Command<T, R>>) -> Self { Self { sender } }
}

impl<T, R> Drop for Stop<T, R> {
    fn drop(&mut self) { let _ = self.sender.send(Command::Drop); }
}
