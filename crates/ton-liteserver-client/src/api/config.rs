use crate::tl::{Int31, LiteServerConfigInfo, LiteServerGetConfigAll, TonNodeBlockIdExt};
use crate::tlb::ext_blk_ref::ExtBlkRef;
use crate::tlb::global_version::GlobalVersion;
use crate::tlb::hashmap::{Hashmap, HashmapLookup};
use crate::tlb::mc_state_extra::McStateExtraInfo;
use crate::tlb::merkle_proof::MerkleProof;
use crate::tlb::shard_state::ShardStateUnsplit;
use crate::tlb::vm_stack::{VmStackValue, VmStkTuple};
use anyhow::anyhow;
use num_bigint::BigUint;
use toner::tlb::bits::de::unpack_fully;
use toner::tlb::{BoC, Cell};
use tower::{Service, ServiceExt};

const GET_CONFIG_ALL_MODE: Int31 = 0x80;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError<E> {
    #[error("failed to fetch masterchain config")]
    Transport(#[source] E),

    #[error("failed to parse masterchain config")]
    Parse(#[source] anyhow::Error),
}

#[derive(Debug, Clone)]
pub struct MasterchainConfig {
    block_id: TonNodeBlockIdExt,
    config_addr: [u8; 32],
    params: Hashmap<u32, Cell, 32>,
    state_extra: McStateExtraInfo,
}

impl MasterchainConfig {
    pub const fn block_id(&self) -> &TonNodeBlockIdExt {
        &self.block_id
    }

    pub const fn config_addr(&self) -> &[u8; 32] {
        &self.config_addr
    }

    pub const fn params(&self) -> &Hashmap<u32, Cell, 32> {
        &self.params
    }

    pub const fn state_extra(&self) -> &McStateExtraInfo {
        &self.state_extra
    }

    pub fn global_version(&self) -> anyhow::Result<u32> {
        match self.params.lookup(&8_u32) {
            HashmapLookup::Found(cell) => {
                let version: GlobalVersion = unpack_fully(cell.data.as_bitslice(), ())?;
                Ok(version.version)
            }
            HashmapLookup::Absent => Ok(0),
            HashmapLookup::Pruned(_) => Err(anyhow!("global version config parameter is pruned")),
        }
    }

    pub fn prev_blocks_info(&self) -> anyhow::Result<VmStackValue> {
        let current_seqno = u32::try_from(self.block_id.seqno)
            .map_err(|_| anyhow!("invalid masterchain block seqno: {}", self.block_id.seqno))?;
        let mut last_mc_blocks = vec![block_id_to_tuple(
            self.block_id.workchain,
            self.block_id.shard,
            current_seqno,
            self.block_id.root_hash,
            self.block_id.file_hash,
        )];

        let mut seqno = current_seqno;
        while seqno > 0 && last_mc_blocks.len() < 16 {
            seqno -= 1;
            last_mc_blocks.push(self.old_block_to_tuple(seqno)?);
        }

        let prev_key_block = if self.state_extra.after_key_block {
            block_id_to_tuple(
                self.block_id.workchain,
                self.block_id.shard,
                current_seqno,
                self.block_id.root_hash,
                self.block_id.file_hash,
            )
        } else {
            let block = self
                .state_extra
                .last_key_block
                .as_ref()
                .ok_or_else(|| anyhow!("masterchain config has no previous key block"))?;
            ext_block_to_tuple(block)
        };

        let mut result = vec![tuple(last_mc_blocks), prev_key_block];
        if self.global_version()? >= 9 {
            let mut last_mc_blocks_100 = Vec::with_capacity(16);
            let mut seqno = current_seqno / 100 * 100;
            while last_mc_blocks_100.len() < 16 {
                last_mc_blocks_100.push(if seqno == current_seqno {
                    block_id_to_tuple(
                        self.block_id.workchain,
                        self.block_id.shard,
                        current_seqno,
                        self.block_id.root_hash,
                        self.block_id.file_hash,
                    )
                } else {
                    self.old_block_to_tuple(seqno)?
                });
                if seqno < 100 {
                    break;
                }
                seqno -= 100;
            }
            result.push(tuple(last_mc_blocks_100));
        }

        Ok(tuple(result))
    }

    fn old_block_to_tuple(&self, seqno: u32) -> anyhow::Result<VmStackValue> {
        match self.state_extra.prev_blocks.lookup(seqno)? {
            HashmapLookup::Found(block) => Ok(ext_block_to_tuple(&block.blk_ref)),
            HashmapLookup::Absent => Err(anyhow!("old masterchain block {seqno} is absent")),
            HashmapLookup::Pruned(_) => Err(anyhow!("old masterchain block {seqno} is pruned")),
        }
    }
}

fn ext_block_to_tuple(block: &ExtBlkRef) -> VmStackValue {
    block_id_to_tuple(-1, i64::MIN, block.seq_no, block.root_hash, block.file_hash)
}

fn block_id_to_tuple(
    workchain: i32,
    shard: i64,
    seqno: u32,
    root_hash: [u8; 32],
    file_hash: [u8; 32],
) -> VmStackValue {
    tuple(vec![
        VmStackValue::TinyInt {
            value: i64::from(workchain),
        },
        unsigned_integer(shard as u64),
        VmStackValue::TinyInt {
            value: i64::from(seqno),
        },
        VmStackValue::Int {
            value: BigUint::from_bytes_be(&root_hash),
        },
        VmStackValue::Int {
            value: BigUint::from_bytes_be(&file_hash),
        },
    ])
}

fn unsigned_integer(value: u64) -> VmStackValue {
    match i64::try_from(value) {
        Ok(value) => VmStackValue::TinyInt { value },
        Err(_) => VmStackValue::Int {
            value: BigUint::from(value),
        },
    }
}

fn tuple(items: Vec<VmStackValue>) -> VmStackValue {
    VmStackValue::Tuple {
        tuple: VmStkTuple(items),
    }
}

impl TryFrom<LiteServerConfigInfo> for MasterchainConfig {
    type Error = anyhow::Error;

    fn try_from(response: LiteServerConfigInfo) -> Result<Self, Self::Error> {
        let proof = BoC::deserialize(&response.config_proof)?;
        let root = proof
            .single_root()
            .ok_or_else(|| anyhow!("single config proof root expected"))?;
        let state: ShardStateUnsplit = root.parse_fully_as::<_, MerkleProof<_>>(())?;
        let extra = state
            .custom
            .ok_or_else(|| anyhow!("masterchain state has no McStateExtra"))?;

        Ok(Self {
            block_id: response.id,
            config_addr: extra.config.config_addr,
            params: extra.config.config,
            state_extra: extra.state_extra,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ConfigClient<T> {
    inner: T,
}

impl<T> ConfigClient<T> {
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

impl<T> ConfigClient<T>
where
    T: Service<LiteServerGetConfigAll, Response = LiteServerConfigInfo>,
{
    pub async fn get_all(
        &mut self,
        block_id: TonNodeBlockIdExt,
    ) -> Result<MasterchainConfig, ConfigError<T::Error>> {
        let response = self
            .get_all_raw(block_id)
            .await
            .map_err(ConfigError::Transport)?;

        MasterchainConfig::try_from(response).map_err(ConfigError::Parse)
    }

    pub async fn get_all_raw(
        &mut self,
        block_id: TonNodeBlockIdExt,
    ) -> Result<LiteServerConfigInfo, T::Error> {
        self.inner
            .ready()
            .await?
            .call(LiteServerGetConfigAll {
                mode: GET_CONFIG_ALL_MODE,
                id: block_id,
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigClient, ConfigError};
    use crate::tl::{LiteServerConfigInfo, LiteServerGetConfigAll, TonNodeBlockIdExt};
    use std::convert::Infallible;
    use std::io;
    use std::sync::{Arc, Mutex};
    use tower::service_fn;

    #[tokio::test]
    async fn should_get_all_raw_config_with_previous_blocks() {
        let block_id = block_id();
        let expected = LiteServerConfigInfo {
            mode: 0x80,
            id: block_id.clone(),
            state_proof: vec![1, 2, 3],
            config_proof: vec![4, 5, 6],
        };
        let request = Arc::new(Mutex::new(None));
        let captured_request = Arc::clone(&request);
        let service = service_fn(move |request: LiteServerGetConfigAll| {
            *captured_request.lock().unwrap() = Some(request);
            let response = expected.clone();
            async move { Ok::<_, Infallible>(response) }
        });
        let mut client = ConfigClient::new(service);

        let response = client.get_all_raw(block_id.clone()).await.unwrap();

        assert_eq!(response.id, block_id);
        let request = request.lock().unwrap().take().unwrap();
        assert_eq!(request.mode, 0x80);
        assert_eq!(request.id, block_id);
    }

    #[tokio::test]
    async fn should_return_parse_error_for_invalid_config_proof() {
        let service = service_fn(|_request: LiteServerGetConfigAll| async {
            Ok::<_, Infallible>(LiteServerConfigInfo {
                mode: 0x80,
                id: block_id(),
                state_proof: Vec::new(),
                config_proof: b"invalid boc".to_vec(),
            })
        });
        let mut client = ConfigClient::new(service);

        let result = client.get_all(block_id()).await;

        assert!(matches!(result, Err(ConfigError::Parse(_))));
    }

    #[tokio::test]
    async fn should_return_transport_error() {
        let service = service_fn(|_request: LiteServerGetConfigAll| async {
            Err::<LiteServerConfigInfo, _>(io::Error::other("transport failed"))
        });
        let mut client = ConfigClient::new(service);

        let result = client.get_all(block_id()).await;

        assert!(matches!(result, Err(ConfigError::Transport(_))));
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
    use super::ConfigClient;
    use crate::client::LiteServerClient;
    use crate::tl::LiteServerGetMasterchainInfo;
    use crate::tlb::vm_stack::VmStackValue;
    use anyhow::anyhow;
    use testcontainers_ton::LocalLiteServer;
    use tower::ServiceExt;

    #[tokio::test]
    async fn should_get_all_config_from_lite_server() -> anyhow::Result<()> {
        let server = LocalLiteServer::shared().await?;
        let client = LiteServerClient::connect(server.addr(), server.server_key()).await?;
        let masterchain = client
            .clone()
            .oneshot(LiteServerGetMasterchainInfo::default())
            .await
            .map_err(|error| anyhow!(error))?;
        let block_id = masterchain.last;
        let mut config = ConfigClient::new(client);

        let parsed = config
            .get_all(block_id.clone())
            .await
            .map_err(|error| anyhow!(error))?;
        let global_version = parsed.global_version()?;
        let prev_blocks_info = parsed.prev_blocks_info()?;
        let VmStackValue::Tuple { tuple: outer } = &prev_blocks_info else {
            unreachable!("prev_blocks_info must be a tuple")
        };
        let VmStackValue::Tuple {
            tuple: last_mc_blocks,
        } = &outer.0[0]
        else {
            unreachable!("last_mc_blocks must be a tuple")
        };

        assert_eq!(parsed.block_id(), &block_id);
        assert!(!parsed.state_extra().prev_blocks.0.hashmap.is_empty());
        assert_eq!(outer.0.len(), if global_version >= 9 { 3 } else { 2 });
        assert_eq!(
            last_mc_blocks.0.len(),
            usize::try_from(block_id.seqno + 1)?.min(16)
        );
        Ok(())
    }
}
