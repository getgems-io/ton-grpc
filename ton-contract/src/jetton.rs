use async_trait::async_trait;
use num_bigint::BigUint;
use toner::tlb::{ton::MsgAddress};
use toner::tlb::r#as::Data;

use crate::{TonContract, TonContractError, TvmBoxedStackEntryExt};

pub struct JettonWalletData {
    pub balance: BigUint,
    pub owner: MsgAddress,
    pub master: MsgAddress,
    // TODO: jetton_wallet_code
}

#[async_trait]
pub trait JettonWalletContract {
    async fn get_wallet_data(&self) -> Result<JettonWalletData, TonContractError>;
}

#[async_trait]
impl JettonWalletContract for TonContract {
    async fn get_wallet_data(&self) -> Result<JettonWalletData, TonContractError> {
        let [balance, owner, master, _jetton_wallet_code] = self
            .run_get_method("get_wallet_data", [].into())
            .await?
            .try_into()?;

        Ok(JettonWalletData {
            balance: balance.into_number()?,
            owner: owner.parse_cell_fully_as::<_, Data>()?,
            master: master.parse_cell_fully_as::<_, Data>()?,
        })
    }
}
