use futures::StreamExt;
use tokio::time::Instant;
use tonlibjson_client::block::RawTransaction;

use tonlibjson_client::ton::TonClientBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut ton = TonClientBuilder::default().await?;
    ton.ready().await?;

    let now = Instant::now();

    let address = "EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS";

    let total_value: i64 = ton.get_account_tx_range_unordered(address, ..).await?
        .filter_map(|tx| async {
            let tx: RawTransaction = tx.unwrap();
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
    println!("Time: {:?}", timing);

    Ok(())
}
