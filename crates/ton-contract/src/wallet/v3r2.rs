use std::sync::Arc;

use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use toner::{
    contracts::wallet::{PUBLIC_KEY_LENGTH, WalletVersion},
    ton::{BagOfCells, Cell, UnixTimestamp, action::SendMsgAction},
};
use toner_tlb_macros::{CellDeserialize, CellSerialize};

// TODO[akostylev0]: move to toner

lazy_static! {
    static ref WALLET_V3R2_CODE_CELL: Arc<Cell> = {
        BagOfCells::parse_base64(include_str!("./wallet_v3r2.code"))
            .unwrap()
            .into_single_root()
            .expect("code BoC must be single root")
    };
}

pub struct V3R2;

impl WalletVersion for V3R2 {
    type Data = WalletV3R2Data;
    type SignBody = WalletV3R2SignBody;
    type ExternalMsgBody = WalletV3R2ExternalBody;

    const DEFAULT_WALLET_ID: u32 = 0x29a9a317;

    fn code() -> Arc<Cell> {
        WALLET_V3R2_CODE_CELL.clone()
    }

    fn init_data(wallet_id: u32, pubkey: [u8; PUBLIC_KEY_LENGTH]) -> Self::Data {
        WalletV3R2Data {
            seqno: 0,
            wallet_id,
            pubkey,
        }
    }

    fn create_sign_body(
        wallet_id: u32,
        expire_at: DateTime<Utc>,
        seqno: u32,
        msgs: impl IntoIterator<Item = SendMsgAction>,
    ) -> Self::SignBody {
        WalletV3R2SignBody {
            wallet_id,
            expire_at,
            seqno,
            msgs: msgs.into_iter().collect(),
        }
    }

    fn wrap_signed_external(body: Self::SignBody, signature: [u8; 64]) -> Self::ExternalMsgBody {
        WalletV3R2ExternalBody { signature, body }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, CellSerialize, CellDeserialize)]
pub struct WalletV3R2Data {
    #[tlb(bits)]
    pub seqno: u32,
    #[tlb(bits)]
    pub wallet_id: u32,
    #[tlb(bits)]
    pub pubkey: [u8; PUBLIC_KEY_LENGTH],
}

#[derive(Debug, Clone, PartialEq, Eq, CellSerialize, CellDeserialize)]
pub struct WalletV3R2SignBody {
    #[tlb(bits)]
    pub wallet_id: u32,
    #[tlb(bits, as = "UnixTimestamp")]
    pub expire_at: DateTime<Utc>,
    #[tlb(bits)]
    pub seqno: u32,
    #[tlb(iter)]
    pub msgs: Vec<SendMsgAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, CellSerialize, CellDeserialize)]
pub struct WalletV3R2ExternalBody {
    #[tlb(bits)]
    pub signature: [u8; 64],
    #[tlb(cell)]
    pub body: WalletV3R2SignBody,
}

#[cfg(test)]
mod tests {
    use toner::ton::{
        BagOfCellsArgs, BoC,
        bits::{de::unpack_fully, ser::pack},
    };

    use super::*;

    #[test]
    fn check_code() {
        let packed = pack(
            BoC::from_root(WALLET_V3R2_CODE_CELL.clone()),
            BagOfCellsArgs {
                has_idx: false,
                has_crc32c: true,
            },
        )
        .unwrap();

        let unpacked: BoC = unpack_fully(&packed, ()).unwrap();
        let got: Cell = unpacked.single_root().unwrap().parse_fully(()).unwrap();

        assert_eq!(&got, WALLET_V3R2_CODE_CELL.as_ref());
    }
}
