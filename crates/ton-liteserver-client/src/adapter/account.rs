use crate::client::LiteServerClient;
use crate::tl::{
    Int256, LiteServerAccountId, LiteServerGetAccountState, LiteServerGetOneTransaction,
    TonNodeBlockIdExt,
};
use crate::tlb::account::Account;
use crate::tlb::account_state::AccountState as TlbAccountState;
use crate::tlb::account_storage::AccountStorage;
use crate::tlb::merkle_proof::MerkleProof;
use crate::tlb::shard_state::ShardStateUnsplit;
use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as base64_standard;
use num_bigint::BigUint;
use std::sync::Arc;
use ton_address::SmartContractAddress;
use ton_tower::response::{AccountState, BlockIdExt, Cell as TonCell, TransactionId};
use toner::tlb::{BagOfCellsArgs, BoC, Cell};
use tower::ServiceExt;

pub(super) const DEFAULT_TX_BATCH: i32 = 16;

pub(super) async fn get_account_state_inner(
    client: LiteServerClient,
    address: SmartContractAddress,
    block_id: TonNodeBlockIdExt,
) -> anyhow::Result<AccountState> {
    let response = fetch_account_state_raw(&client, &address, block_id).await?;

    account_state_from_response(response)
}

pub(super) async fn get_shard_account_cell_inner(
    client: LiteServerClient,
    address: SmartContractAddress,
    block_id: TonNodeBlockIdExt,
) -> anyhow::Result<TonCell> {
    let response = fetch_account_state_raw(&client, &address, block_id).await?;

    shard_account_cell_from_response(response)
}

pub(super) fn account_state_request(
    address: &SmartContractAddress,
    block_id: TonNodeBlockIdExt,
) -> LiteServerGetAccountState {
    LiteServerGetAccountState {
        id: block_id,
        account: LiteServerAccountId {
            workchain: address.workchain_id(),
            id: *address.to_internal(),
        },
    }
}

pub(super) fn account_state_from_response(
    response: crate::tl::LiteServerAccountState,
) -> anyhow::Result<AccountState> {
    // TODO verify ShardAccount inclusion via ShardAccounts dict traversal (needs MaybePruned)
    verify_account_proofs(&response)?;

    let block_id_out: BlockIdExt = response.shardblk.clone().into();
    let sync_utime = extract_gen_utime_from_proof(&response.proof)?;

    if response.state.is_empty() {
        return Ok(AccountState {
            balance: None,
            code: String::new(),
            data: String::new(),
            frozen_hash: String::new(),
            last_transaction_id: None,
            block_id: block_id_out,
            sync_utime,
        });
    }

    let acc_boc = BoC::deserialize(&response.state)?;
    let acc_root = acc_boc
        .single_root()
        .ok_or_else(|| anyhow!("account state: single root expected"))?
        .clone();

    let account: Account = acc_root.parse_fully(())?;
    let (balance, code, data, frozen_hash) = account_components(&account)?;

    Ok(AccountState {
        balance,
        code,
        data,
        frozen_hash,
        // TODO[akostylev0]: extract last_transaction_id from ShardAccount in `proof` (needs MaybePruned)
        last_transaction_id: None,
        block_id: block_id_out,
        sync_utime,
    })
}

pub(super) fn shard_account_cell_from_response(
    response: crate::tl::LiteServerAccountState,
) -> anyhow::Result<TonCell> {
    // TODO verify ShardAccount inclusion via ShardAccounts dict traversal (needs MaybePruned)
    verify_account_proofs(&response)?;

    Ok(TonCell {
        bytes: base64_standard.encode(&response.state),
    })
}

async fn fetch_account_state_raw(
    client: &LiteServerClient,
    address: &SmartContractAddress,
    block_id: TonNodeBlockIdExt,
) -> anyhow::Result<crate::tl::LiteServerAccountState> {
    client
        .clone()
        .oneshot(account_state_request(address, block_id))
        .await
        .map_err(|e| anyhow!(e))
}

pub(super) async fn get_emulator_state_inner(
    client: &LiteServerClient,
    address: &SmartContractAddress,
    block_id: TonNodeBlockIdExt,
) -> anyhow::Result<(Arc<Cell>, Arc<Cell>)> {
    let response = fetch_account_state_raw(client, address, block_id).await?;
    verify_account_proofs(&response)?;
    let account = BoC::deserialize(&response.state)?
        .single_root()
        .ok_or_else(|| anyhow!("account state: single root expected"))?
        .parse_fully::<Account>(())?;

    let Account::Account { storage, .. } = account else {
        return Err(anyhow!("cannot run get method for a missing account"));
    };
    let TlbAccountState::Active { state_init } = storage.state else {
        return Err(anyhow!("cannot run get method for an inactive account"));
    };
    let code = state_init
        .code
        .ok_or_else(|| anyhow!("active account has no code"))?;
    let data = state_init
        .data
        .ok_or_else(|| anyhow!("active account has no data"))?;

    Ok((code, data))
}

pub(super) async fn lookup_block_by_transaction(
    client: &LiteServerClient,
    mc_last: TonNodeBlockIdExt,
    address: &SmartContractAddress,
    tx: &TransactionId,
) -> anyhow::Result<TonNodeBlockIdExt> {
    let hash: Int256 = decode_tx_hash(&tx.hash)?;
    let account = LiteServerAccountId {
        workchain: address.workchain_id(),
        id: *address.to_internal(),
    };

    let response = client
        .clone()
        .oneshot(LiteServerGetOneTransaction {
            id: mc_last,
            account,
            lt: tx.lt,
        })
        .await
        .map_err(|e| anyhow!(e))?;

    // TODO verify transaction hash matches `tx.hash` via proof (TransactionRef in shard state)
    let _ = hash;

    Ok(response.id)
}

// Verifies binding of proofs to the response by structurally parsing every Merkle proof
// root and checking that at least one root in `proof` references shardblk.root_hash, and at
// least one root in `shard_proof` references the requested mc block root_hash.
// TODO[akostylev0]: traverse ShardAccounts dict inside proof to confirm account inclusion (needs MaybePruned).
fn verify_account_proofs(response: &crate::tl::LiteServerAccountState) -> anyhow::Result<()> {
    require_proof_binds_to(
        &response.proof,
        &response.shardblk.root_hash,
        "account proof",
    )?;
    if !response.shard_proof.is_empty() {
        require_proof_binds_to(
            &response.shard_proof,
            &response.id.root_hash,
            "account shard_proof",
        )?;
    }
    Ok(())
}

fn extract_gen_utime_from_proof(proof_bytes: &[u8]) -> anyhow::Result<i64> {
    let boc = BoC::deserialize(proof_bytes)?;
    for root in boc.roots() {
        let Ok(state) = root.parse_fully_as::<ShardStateUnsplit, MerkleProof<_>>(()) else {
            continue;
        };
        return Ok(state.gen_utime as i64);
    }
    Err(anyhow!(
        "proof contains no ShardStateUnsplit root (expected tag 0x9023afe2)"
    ))
}

fn require_proof_binds_to(
    proof_bytes: &[u8],
    expected_root_hash: &[u8; 32],
    what: &'static str,
) -> anyhow::Result<()> {
    if proof_bytes.is_empty() {
        return Err(anyhow!("{}: empty proof", what));
    }
    let boc = BoC::deserialize(proof_bytes)?;
    let roots = boc.roots();
    if roots.is_empty() {
        return Err(anyhow!("{}: no roots", what));
    }

    let mut virtual_hashes = Vec::with_capacity(roots.len());
    for root in roots {
        let mp: MerkleProof<Cell> = root.parse_fully(())?;
        virtual_hashes.push(mp.virtual_hash);
    }

    if !virtual_hashes.iter().any(|h| h == expected_root_hash) {
        return Err(anyhow!(
            "{}: no root binds to expected root_hash {} (got: {:?})",
            what,
            hex::encode(expected_root_hash),
            virtual_hashes.iter().map(hex::encode).collect::<Vec<_>>()
        ));
    }

    Ok(())
}

fn account_components(account: &Account) -> anyhow::Result<(Option<i64>, String, String, String)> {
    match account {
        Account::None => Ok((None, String::new(), String::new(), String::new())),
        Account::Account { storage, .. } => {
            let balance = balance_to_i64(&storage.balance.grams);
            let (code, data, frozen_hash) = storage_state_components(storage)?;

            Ok((Some(balance), code, data, frozen_hash))
        }
    }
}

fn balance_to_i64(v: &BigUint) -> i64 {
    v.try_into().unwrap_or(i64::MAX)
}

fn storage_state_components(storage: &AccountStorage) -> anyhow::Result<(String, String, String)> {
    match &storage.state {
        TlbAccountState::Uninit => Ok((String::new(), String::new(), String::new())),
        TlbAccountState::Frozen { state_hash } => Ok((
            String::new(),
            String::new(),
            base64_standard.encode(state_hash),
        )),
        TlbAccountState::Active { state_init } => {
            let code = encode_optional_cell_boc(state_init.code.as_ref())?;
            let data = encode_optional_cell_boc(state_init.data.as_ref())?;
            Ok((code, data, String::new()))
        }
    }
}

fn encode_optional_cell_boc(cell: Option<&Arc<Cell>>) -> anyhow::Result<String> {
    let Some(cell) = cell else {
        return Ok(String::new());
    };
    let bytes = BoC::from_root(Arc::clone(cell))
        .serialize(BagOfCellsArgs {
            has_crc32c: true,
            ..BagOfCellsArgs::default()
        })
        .map_err(|e| anyhow!("BoC serialize failed: {e}"))?;
    Ok(base64_standard.encode(bytes))
}

pub(super) fn decode_tx_hash(hash_b64: &str) -> anyhow::Result<Int256> {
    let raw = base64_standard
        .decode(hash_b64)
        .map_err(|e| anyhow!("invalid base64 tx hash: {}", e))?;
    raw.as_slice()
        .try_into()
        .map_err(|_| anyhow!("tx hash must be 32 bytes, got {}", raw.len()))
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::adapter::LiteServerAdapter;
    use crate::client::LiteServerClient;
    use crate::tl::LiteServerGetMasterchainInfo;
    use std::str::FromStr;
    use testcontainers_ton::{LocalLiteServer, SharedLiteServer};
    use ton_tower::request::*;
    use tower::ServiceExt;
    use tracing_test::traced_test;

    const CONFIG_MASTER_ADDR: &str =
        "-1:5555555555555555555555555555555555555555555555555555555555555555";
    const FAUCET_WALLET_ADDR: &str =
        "-1:22f53b7d9aba2cef44755f7078b01614cd4dde2388a1729c2c386cf8f9898afe";
    const FAUCET_WALLET_BASECHAIN_ADDR: &str =
        "0:1da77f0269bbbb76c862ea424b257df63bd1acb0d4eb681b68c9aadfbf553b93";

    #[tokio::test]
    #[traced_test]
    async fn get_account_state_returns_balance_for_config_master() -> anyhow::Result<()> {
        let (adapter, _server) = setup().await?;
        let address = SmartContractAddress::from_str(CONFIG_MASTER_ADDR)?;

        let state = adapter.oneshot(GetAccountState { address }).await?;

        assert!(state.balance.is_some());
        assert!(state.balance.unwrap() >= 0);
        assert!(!state.block_id.root_hash.is_empty());
        assert_eq!(state.block_id.workchain, -1);
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn get_account_state_returns_balance_for_faucet_wallet() -> anyhow::Result<()> {
        let (adapter, _server) = setup().await?;
        let address = SmartContractAddress::from_str(FAUCET_WALLET_ADDR)?;

        let state = adapter.oneshot(GetAccountState { address }).await?;

        let balance = state.balance.expect("faucet wallet must have balance");
        assert!(balance > 0, "faucet wallet should have positive balance");
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn get_account_state_returns_code_and_data_for_active_wallet() -> anyhow::Result<()> {
        let (adapter, _server) = setup().await?;
        let address = SmartContractAddress::from_str(FAUCET_WALLET_ADDR)?;

        let state = adapter.oneshot(GetAccountState { address }).await?;

        assert!(
            !state.code.is_empty(),
            "active wallet must have code, got empty"
        );
        assert!(
            !state.data.is_empty(),
            "active wallet must have data, got empty"
        );
        assert!(
            state.frozen_hash.is_empty(),
            "active wallet must not be frozen"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn get_account_state_returns_recent_sync_utime() -> anyhow::Result<()> {
        let (adapter, _server) = setup().await?;
        let address = SmartContractAddress::from_str(CONFIG_MASTER_ADDR)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        let state = adapter.oneshot(GetAccountState { address }).await?;

        assert!(
            state.sync_utime > 0,
            "sync_utime must be non-zero, got {}",
            state.sync_utime
        );
        let drift = now - state.sync_utime;
        assert!(
            drift.abs() < 600,
            "sync_utime drift from local clock is too large: now={now}, sync_utime={}, drift={drift}s",
            state.sync_utime
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn get_account_state_on_block_resolves_shard_for_basechain() -> anyhow::Result<()> {
        let (adapter, _server) = setup().await?;
        let address = SmartContractAddress::from_str(FAUCET_WALLET_BASECHAIN_ADDR)?;
        let mc = adapter
            .inner()
            .clone()
            .oneshot(LiteServerGetMasterchainInfo::default())
            .await
            .map_err(|e| anyhow!(e))?;
        let block_id: BlockIdExt = mc.last.into();

        let state = adapter
            .oneshot(GetAccountStateOnBlock { address, block_id })
            .await?;

        assert_eq!(state.block_id.workchain, 0);
        assert!(!state.block_id.root_hash.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn get_shard_account_cell_returns_non_empty_state() -> anyhow::Result<()> {
        let (adapter, _server) = setup().await?;
        let address = SmartContractAddress::from_str(FAUCET_WALLET_ADDR)?;

        let cell = adapter.oneshot(GetShardAccountCell { address }).await?;

        assert!(
            !cell.bytes.is_empty(),
            "shard account cell must not be empty"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn get_account_state_for_unknown_address_returns_empty() -> anyhow::Result<()> {
        let (adapter, _server) = setup().await?;
        let address = SmartContractAddress::raw(0, [0xab; 32]);

        let state = adapter.oneshot(GetAccountState { address }).await?;

        assert!(state.balance.is_none());
        assert!(state.code.is_empty());
        Ok(())
    }

    async fn setup() -> anyhow::Result<(LiteServerAdapter, SharedLiteServer)> {
        let server = LocalLiteServer::shared().await?;
        let client = LiteServerClient::connect(server.addr(), server.server_key()).await?;
        Ok((LiteServerAdapter::new(client), server))
    }
}
