use tonlibjson_client::block::{InternalTransactionId, RawTransaction};
use tonlibjson_client::ton::TonClient;
use futures::StreamExt;
use tracing::debug;
use tracing_test::traced_test;

#[tokio::test]
#[traced_test]
async fn get_account_tx_stream_starts_from() -> anyhow::Result<()> {
    let client = TonClient::from_env().await?;

    let address = "EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS".to_owned();
    let hash = "752Szayka+Eh54Zvco5l84d6WL+zJFmyh1wqRxD08Uo=";
    let lt = 33756943000007;
    let tx = InternalTransactionId {
        hash: hash.to_owned(),
        lt: lt.to_owned()
    };

    let transaction_list: Vec<anyhow::Result<RawTransaction>> = client
        .get_account_tx_stream_from(address, tx.clone())
        .take(1)
        .collect()
        .await;

    debug!("{:#?}", transaction_list);

    assert_eq!(transaction_list.len(), 1);
    assert_eq!(transaction_list[0].as_ref().unwrap().transaction_id, tx);

    Ok(())
}
