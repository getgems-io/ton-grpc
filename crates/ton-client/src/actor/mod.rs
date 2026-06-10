use tokio::task::JoinHandle;
use tokio_util::task::AbortOnDropHandle;

pub trait Actor: Sized + Send + 'static {
    type Output: Send;

    fn run(self) -> impl Future<Output = <Self as Actor>::Output> + Send;

    fn spawn(self) -> JoinHandle<Self::Output> {
        tokio::spawn(self.run())
    }

    fn spawn_cancellable(self) -> AbortOnDropHandle<Self::Output> {
        AbortOnDropHandle::new(self.spawn())
    }
}
