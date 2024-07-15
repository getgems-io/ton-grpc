pub mod cancellable_actor;

use tokio::task::JoinHandle;

pub trait Actor: Sized + Send + 'static {
    type Output: Send;

    fn run(self) -> impl std::future::Future<Output = <Self as Actor>::Output> + Send;

    fn spawn(self) -> JoinHandle<Self::Output> {
        tokio::spawn(self.run())
    }
}
