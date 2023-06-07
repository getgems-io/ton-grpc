use futures::StreamExt;
use tokio::time::Instant;
use tonlibjson_client::block::RawTransaction;

use tonlibjson_client::ton::TonClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut ton = TonClient::from_env().await?;

    ton.ready().await?;

    let now = Instant::now();

    let address = "EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS";

    let total_value: i64 = ton.get_account_tx_range_unordered(address, ..).await?
        .filter_map(|tx| async {
            let tx: RawTransaction = tx.unwrap();

            tracing::info!(lt = tx.transaction_id.lt);
            if let Some(msg) = tx.out_msgs.first() {
                Some(-msg.value - tx.fee)
            } else {
                Some(tx.in_msg.value - tx.fee)
            }
        })
        .collect::<Vec<i64>>()
        .await
        .iter()
        .sum();

    let timing = (Instant::now() - now).as_secs();

    println!("Total value: {:?}", total_value / 1000000000);
    println!("Time: {:?}", timing);

    Ok(())
}
