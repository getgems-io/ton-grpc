use std::str::FromStr;
use std::time::Duration;

use chrono::Utc;
use ton_address::SmartContractAddress;
use ton_client::{AccountClient, MessageClient};
use toner::contracts::wallet::{Wallet, mnemonic::Mnemonic};
use toner::ton::action::SendMsgAction;
use toner::ton::bits::ser::pack;
use toner::ton::message::Message;
use toner::ton::ser::{CellSerialize, CellSerializeExt};
use toner::ton::{BagOfCellsArgs, BoC, MsgAddress};

use crate::TonContract;
use crate::wallet::WalletContract;
use crate::wallet::v3r2::V3R2;

#[tokio::test]
async fn should_get_balance_of_preinstalled_wallet() -> anyhow::Result<()> {
    let (_server, client) = setup().await?;
    let wallet = wallet_from_mnemonic(GENESIS_MNEMONIC, -1, 42);
    let expected_address = SmartContractAddress::from_str(
        "-1:6744e92c6f71c776fbbcef299e31bf76f39c245cd56f2075b89c6a22026b4131",
    )?;

    let wallet_address = SmartContractAddress::from_str(&wallet.address().to_hex())?;
    let state = client.get_account_state(&wallet_address).await?;

    assert_eq!(wallet_address, expected_address);
    assert!(state.balance.unwrap_or(0) > 0);

    Ok(())
}

#[tokio::test]
async fn should_get_seqno() -> anyhow::Result<()> {
    let (_server, client) = setup().await?;
    let wallet = wallet_from_mnemonic(GENESIS_MNEMONIC, -1, 42);
    let contract = TonContract::new(client, wallet.address());

    let seqno = contract.seqno().await?;

    assert_eq!(seqno, 0);

    Ok(())
}

#[tokio::test]
async fn should_send_ton_between_wallets() -> anyhow::Result<()> {
    let (_server, client) = setup().await?;
    let sender_wallet = wallet_from_mnemonic(GENESIS_MNEMONIC, -1, 42);
    let receiver_wallet = wallet_from_mnemonic(VALIDATOR1_MNEMONIC, -1, 42);
    let sender_smc_address = SmartContractAddress::from_str(&sender_wallet.address().to_hex())?;
    let sender_msg_address = MsgAddress::from_str(&sender_wallet.address().to_hex())?;
    let receiver_smc_address = SmartContractAddress::from_str(&receiver_wallet.address().to_hex())?;
    let contract = TonContract::new(client.clone(), sender_msg_address);
    let seqno = contract.seqno().await?;
    let sender_balance_before = client
        .get_account_state(&sender_smc_address)
        .await?
        .balance
        .unwrap_or(0);
    let receiver_balance_before = client
        .get_account_state(&receiver_smc_address)
        .await?
        .balance
        .unwrap_or(0);

    let one_ton: i64 = 1_000_000_000;
    let expire_at = Utc::now() + chrono::Duration::minutes(3);
    let internal_msg = Message::<()>::transfer(
        receiver_wallet.address(),
        toner::ton::currency::ONE_TON.clone(),
        true,
    )
    .normalize()?;
    let external_msg = sender_wallet.create_external_message(
        expire_at,
        seqno,
        [SendMsgAction {
            mode: 3,
            message: internal_msg,
        }],
        false,
    )?;
    client
        .send_message(&message_to_boc_base64(&external_msg))
        .await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    let sender_balance_after = client
        .get_account_state(&sender_smc_address)
        .await?
        .balance
        .unwrap_or(0);
    let receiver_balance_after = client
        .get_account_state(&receiver_smc_address)
        .await?
        .balance
        .unwrap_or(0);
    let sender_spent = sender_balance_before - sender_balance_after;
    let receiver_gained = receiver_balance_after - receiver_balance_before;
    assert!(
        sender_spent >= one_ton,
        "sender should spend at least 1 TON: spent={sender_spent}"
    );
    assert!(
        receiver_gained >= one_ton * 9 / 10,
        "receiver should gain at least 0.9 TON (1 TON minus fees): gained={receiver_gained}"
    );
    let new_seqno = contract.seqno().await?;
    assert_eq!(new_seqno, seqno + 1);

    Ok(())
}

const GENESIS_MNEMONIC: &str = "quantum input cannon actress public limit case torch manage pig wrestle sunny riot midnight mouse romance guitar chat race famous jacket donor empty sad";

const VALIDATOR1_MNEMONIC: &str = "dentist melt vault invest alcohol argue sausage embrace afford verify control credit waste file hope vocal air ahead gesture wage innocent today party salad";

fn wallet_from_mnemonic(mnemonic: &str, workchain: i32, wallet_id: u32) -> Wallet<V3R2> {
    let m: Mnemonic = mnemonic.parse().unwrap();
    let keypair = m.generate_keypair(None).unwrap();
    Wallet::<V3R2>::derive(workchain, keypair, wallet_id).unwrap()
}

fn message_to_boc_base64<T, IC, ID>(msg: &Message<T, IC, ID>) -> String
where
    T: CellSerialize<Args = ()>,
    IC: CellSerialize<Args = ()>,
    ID: CellSerialize<Args = ()>,
{
    let cell = msg.to_cell(()).unwrap();
    let boc = BoC::from_root(cell);
    let bytes = pack(
        boc,
        BagOfCellsArgs {
            has_idx: false,
            has_crc32c: true,
        },
    )
    .unwrap();

    base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        bytes.as_raw_slice(),
    )
}

async fn setup() -> anyhow::Result<(
    testcontainers_ton::LocalLiteServer,
    tonlibjson_client::ton::TonClient,
)> {
    let server = testcontainers_ton::LocalLiteServer::new().await?;
    let mut client =
        tonlibjson_client::ton::TonClientBuilder::from_config(server.config()).build()?;
    client.ready().await?;
    Ok((server, client))
}
