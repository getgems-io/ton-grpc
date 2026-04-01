use futures::StreamExt;
use std::ops::Bound;
use std::str::FromStr;
use tokio::time::Instant;
use ton_address::SmartContractAddress;
use ton_client::{AccountClientExt as _, Transaction};
use tonlibjson_client::ton::TonClientBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut ton = TonClientBuilder::default().build()?;
    ton.ready().await?;

    let now = Instant::now();

    let address =
        SmartContractAddress::from_str("EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS")?;

    let total_value: i64 = ton
        .get_account_tx_range_unordered(&address, (Bound::Unbounded, Bound::Unbounded))
        .await?
        .filter_map(|tx| async {
            let tx: Transaction = tx.unwrap();
            if let Some(msg) = tx.out_msgs.first() {
                Some(-msg.value - tx.fee)
            } else if let Some(msg) = tx.in_msg {
                Some(msg.value - tx.fee)
            } else {
                Some(0)
            }
        })
        .collect::<Vec<i64>>()
        .await
        .iter()
        .sum();

    let timing = (Instant::now() - now).as_secs();

    println!("Total value: {:?}", total_value / 1000000000);
    println!("Time: {timing:?}");

    Ok(())
}
