pub mod account_client;
pub mod account_client_ext;
pub mod block_client;
pub mod block_client_ext;
pub mod message_client;
pub mod smc_client;
pub mod types;

pub use account_client::*;
pub use account_client_ext::*;
pub use block_client::*;
pub use block_client_ext::*;
pub use message_client::*;
pub use smc_client::*;
pub use types::*;

pub trait TonClient: BlockClient + AccountClient + SmcClient + MessageClient {}

impl<T: BlockClient + AccountClient + SmcClient + MessageClient> TonClient for T {}

pub trait TonClientExt: BlockClientExt + AccountClientExt {}

impl<T: BlockClient + AccountClient> TonClientExt for T {}
