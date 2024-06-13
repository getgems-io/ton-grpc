pub mod cancellable_actor;

use tokio::task::JoinHandle;

pub trait Actor: Sized + Send + 'static {
    type Output: Send + 'static;

    fn run(self) -> impl std::future::Future<Output = <Self as Actor>::Output> + Send + 'static;

    fn spawn(self) -> JoinHandle<Self::Output> {
        tokio::spawn(self.run())
    }
}
