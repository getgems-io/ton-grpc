use crate::tl::{
    LiteServerAccountId, LiteServerAccountState, LiteServerGetAccountState, TonNodeBlockIdExt,
};
use crate::tlb::account::Account;
use crate::tlb::account_state::AccountState;
use crate::tlb::merkle_proof::MerkleProof;
use crate::tlb::shard_ident::ShardIdent;
use anyhow::anyhow;
use num_bigint::BigUint;
use std::collections::HashMap;
use ton_address::SmartContractAddress;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::{BoC, Cell};
use tower::{Service, ServiceExt};

#[derive(Debug, thiserror::Error)]
pub enum AccountError<E> {
    #[error("failed to fetch account state")]
    Transport(#[source] E),

    #[error("failed to parse account state")]
    Parse(#[source] anyhow::Error),

    #[error("account does not exist")]
    NotFound,

    #[error("account is uninitialized")]
    Uninitialized,

    #[error("account is frozen")]
    Frozen,

    #[error("active account has no code")]
    MissingCode,

    #[error("active account has no data")]
    MissingData,
}

#[derive(Debug, Clone)]
pub struct ActiveAccount {
    address: SmartContractAddress,
    block_id: TonNodeBlockIdExt,
    gen_utime: u32,
    balance: BigUint,
    extra_currencies: HashMap<u32, BigUint>,
    code: Cell,
    data: Cell,
}

impl ActiveAccount {
    pub const fn address(&self) -> &SmartContractAddress {
        &self.address
    }

    pub const fn block_id(&self) -> &TonNodeBlockIdExt {
        &self.block_id
    }

    pub const fn gen_utime(&self) -> u32 {
        self.gen_utime
    }

    pub const fn balance(&self) -> &BigUint {
        &self.balance
    }

    pub const fn extra_currencies(&self) -> &HashMap<u32, BigUint> {
        &self.extra_currencies
    }

    pub const fn code(&self) -> &Cell {
        &self.code
    }

    pub const fn data(&self) -> &Cell {
        &self.data
    }
}

#[derive(Debug, Clone)]
pub struct AccountClient<T> {
    inner: T,
}

impl<T> AccountClient<T> {
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }

    pub const fn inner(&self) -> &T {
        &self.inner
    }

    pub const fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> AccountClient<T>
where
    T: Service<LiteServerGetAccountState, Response = LiteServerAccountState>,
{
    pub async fn get(
        &mut self,
        address: SmartContractAddress,
        block_id: TonNodeBlockIdExt,
    ) -> Result<ActiveAccount, AccountError<T::Error>> {
        let response = self
            .get_raw(address.clone(), block_id)
            .await
            .map_err(AccountError::Transport)?;

        active_account_from_response(address, response)
    }

    pub async fn get_raw(
        &mut self,
        address: SmartContractAddress,
        block_id: TonNodeBlockIdExt,
    ) -> Result<LiteServerAccountState, T::Error> {
        self.inner
            .ready()
            .await?
            .call(LiteServerGetAccountState {
                id: block_id,
                account: LiteServerAccountId {
                    workchain: address.workchain_id(),
                    id: *address.to_internal(),
                },
            })
            .await
    }
}

fn active_account_from_response<E>(
    address: SmartContractAddress,
    response: LiteServerAccountState,
) -> Result<ActiveAccount, AccountError<E>> {
    if response.state.is_empty() {
        return Err(AccountError::NotFound);
    }
    let boc =
        BoC::deserialize(&response.state).map_err(|error| AccountError::Parse(error.into()))?;
    let root = boc
        .single_root()
        .ok_or_else(|| AccountError::Parse(anyhow!("single account state root expected")))?;
    let account: Account = root
        .parse_fully(())
        .map_err(|error| AccountError::Parse(error.into()))?;
    let Account::Account { storage, .. } = account else {
        return Err(AccountError::NotFound);
    };
    let state_init = match storage.state {
        AccountState::Uninit => return Err(AccountError::Uninitialized),
        AccountState::Frozen { .. } => return Err(AccountError::Frozen),
        AccountState::Active { state_init } => state_init,
    };
    let code = state_init
        .code
        .ok_or(AccountError::MissingCode)
        .map(|cell| (*cell).clone())?;
    let data = state_init
        .data
        .ok_or(AccountError::MissingData)
        .map(|cell| (*cell).clone())?;
    let gen_utime = extract_gen_utime(&response.proof).map_err(AccountError::Parse)?;

    Ok(ActiveAccount {
        address,
        block_id: response.shardblk,
        gen_utime,
        balance: storage.balance.grams,
        extra_currencies: storage.balance.other.0,
        code,
        data,
    })
}

fn extract_gen_utime(proof: &[u8]) -> anyhow::Result<u32> {
    let boc = BoC::deserialize(proof)?;
    for root in boc.roots() {
        let Ok(proof) = root.parse_fully::<MerkleProof<Cell>>(()) else {
            continue;
        };
        let mut parser = proof.virtual_root.parser();
        let Ok(tag) = parser.unpack_as::<u32, toner::tlb::bits::NBits<32>>(()) else {
            continue;
        };
        if tag != 0x9023afe2 {
            continue;
        }
        let _: i32 = parser.unpack(())?;
        let _: ShardIdent = parser.unpack(())?;
        let _: u32 = parser.unpack(())?;
        let _: u32 = parser.unpack(())?;
        return parser.unpack(()).map_err(Into::into);
    }
    Err(anyhow!("account proof contains no ShardStateUnsplit root"))
}

#[cfg(test)]
mod tests {
    use super::{AccountClient, AccountError};
    use crate::tl::{LiteServerAccountState, LiteServerGetAccountState, TonNodeBlockIdExt};
    use std::io;
    use std::sync::{Arc, Mutex};
    use ton_address::SmartContractAddress;
    use tower::service_fn;

    #[tokio::test]
    async fn should_request_raw_account_state() {
        let block_id = block_id();
        let address = SmartContractAddress::raw(0, [3; 32]);
        let expected = account_state(block_id.clone());
        let request = Arc::new(Mutex::new(None));
        let captured_request = Arc::clone(&request);
        let service = service_fn(move |request: LiteServerGetAccountState| {
            *captured_request.lock().unwrap() = Some(request);
            let response = expected.clone();
            async move { Ok::<_, io::Error>(response) }
        });
        let mut client = AccountClient::new(service);

        let response = client
            .get_raw(address.clone(), block_id.clone())
            .await
            .unwrap();

        assert_eq!(response.id, block_id);
        let request = request.lock().unwrap().take().unwrap();
        assert_eq!(request.id, block_id);
        assert_eq!(request.account.workchain, address.workchain_id());
        assert_eq!(request.account.id, *address.to_internal());
    }

    #[tokio::test]
    async fn should_return_transport_error() {
        let service = service_fn(|_request: LiteServerGetAccountState| async {
            Err::<LiteServerAccountState, _>(io::Error::other("transport failed"))
        });
        let mut client = AccountClient::new(service);

        let result = client
            .get(SmartContractAddress::raw(0, [3; 32]), block_id())
            .await;

        assert!(matches!(result, Err(AccountError::Transport(_))));
    }

    #[tokio::test]
    async fn should_return_parse_error_for_invalid_account_boc() {
        let service = service_fn(|_request: LiteServerGetAccountState| async {
            let mut response = account_state(block_id());
            response.state = b"invalid boc".to_vec();
            Ok::<_, io::Error>(response)
        });
        let mut client = AccountClient::new(service);

        let result = client
            .get(SmartContractAddress::raw(0, [3; 32]), block_id())
            .await;

        assert!(matches!(result, Err(AccountError::Parse(_))));
    }

    fn account_state(block_id: TonNodeBlockIdExt) -> LiteServerAccountState {
        LiteServerAccountState {
            id: block_id.clone(),
            shardblk: block_id,
            shard_proof: Vec::new(),
            proof: Vec::new(),
            state: Vec::new(),
        }
    }

    fn block_id() -> TonNodeBlockIdExt {
        TonNodeBlockIdExt {
            workchain: -1,
            shard: i64::MIN,
            seqno: 42,
            root_hash: [1; 32],
            file_hash: [2; 32],
        }
    }
}

#[cfg(test)]
mod integration {
    use super::AccountClient;
    use crate::client::LiteServerClient;
    use crate::tl::LiteServerGetMasterchainInfo;
    use anyhow::anyhow;
    use num_bigint::BigUint;
    use std::str::FromStr;
    use testcontainers_ton::LocalLiteServer;
    use ton_address::SmartContractAddress;
    use tower::ServiceExt;

    const FAUCET_WALLET_ADDR: &str =
        "-1:22f53b7d9aba2cef44755f7078b01614cd4dde2388a1729c2c386cf8f9898afe";

    #[tokio::test]
    async fn should_get_active_account_from_lite_server() -> anyhow::Result<()> {
        let server = LocalLiteServer::shared().await?;
        let client = LiteServerClient::connect(server.addr(), server.server_key()).await?;
        let masterchain = client
            .clone()
            .oneshot(LiteServerGetMasterchainInfo::default())
            .await
            .map_err(|error| anyhow!(error))?;
        let block_id = masterchain.last;
        let address = SmartContractAddress::from_str(FAUCET_WALLET_ADDR)?;
        let mut account_client = AccountClient::new(client);

        let account = account_client
            .get(address.clone(), block_id)
            .await
            .map_err(|error| anyhow!(error))?;

        assert_eq!(account.address(), &address);
        assert!(account.gen_utime() > 0);
        assert!(account.balance() > &BigUint::ZERO);
        assert!(!account.code().data.is_empty() || !account.code().references.is_empty());
        assert!(!account.data().data.is_empty() || !account.data().references.is_empty());
        Ok(())
    }
}
