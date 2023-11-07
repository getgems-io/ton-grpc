use tonlibjson_client::block::{InternalTransactionId, RawTransaction};
use tonlibjson_client::ton::{TonClient, TonClientBuilder};
use futures::StreamExt;
use serial_test::serial;
use tokio::sync::OnceCell;
use tracing::debug;
use tracing_test::traced_test;


static CLIENT: OnceCell<TonClient> = OnceCell::const_new();

async fn client() -> TonClient {
    CLIENT.get_or_init(|| async {
        tracing::info!("ready 1");
        let mut client = TonClientBuilder::default().await.unwrap();
        tracing::info!("ready 2");
        client.ready().await.unwrap();
        tracing::info!("ready 3");

        client
    }).await.clone()
}

#[tokio::test]
#[traced_test]
#[serial]
async fn get_account_tx_stream_starts_from() -> anyhow::Result<()> {
    let mut client = client().await;
    client.ready().await?;

    let address = "EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS".to_owned();
    let hash = "752Szayka+Eh54Zvco5l84d6WL+zJFmyh1wqRxD08Uo=";
    let lt = 33756943000007;
    let tx = InternalTransactionId {
        hash: hash.to_owned(),
        lt: lt.to_owned()
    };

    let transaction_list: Vec<anyhow::Result<RawTransaction>> = client
        .get_account_tx_stream_from(&address, Some(tx.clone()))
        .take(1)
        .collect()
        .await;

    debug!("{:#?}", transaction_list);

    assert_eq!(transaction_list.len(), 1);
    assert_eq!(transaction_list[0].as_ref().unwrap().transaction_id, tx);

    Ok(())
}


#[tokio::test]
#[traced_test]
#[serial]
async fn get_account_tx_stream_contains_only_one_transaction() -> anyhow::Result<()> {
    let mut client = client().await;
    client.ready().await?;

    let address = "EQBO_mAVkaHxt6Ibz7wqIJ_UIDmxZBFcgkk7fvIzkh7l42wO".to_owned();

    let transaction_list: Vec<anyhow::Result<RawTransaction>> = client
        .get_account_tx_stream(&address)
        .take(1)
        .collect()
        .await;

    debug!("{:#?}", transaction_list);

    assert_eq!(transaction_list.len(), 1);

    Ok(())
}


#[tokio::test]
#[traced_test]
#[serial]
async fn get_block_tx_stream_correct() -> anyhow::Result<()> {
    let mut client = client().await;
    client.ready().await?;

    let block = client.look_up_block_by_seqno(0, -9223372036854775808, 34716987).await?;

    let len = client.get_block_tx_stream(&block, false)
        .count()
        .await;

    assert_eq!(len, 512);

    Ok(())
}

#[tokio::test]
#[traced_test]
#[serial]
async fn get_block_tx_stream_reverse_correct() -> anyhow::Result<()> {
    let mut client = client().await;
    client.ready().await?;

    let block = client.look_up_block_by_seqno(0, -9223372036854775808, 34716987).await?;

    let len = client.get_block_tx_stream(&block, true)
        .count()
        .await;

    assert_eq!(len, 512);

    Ok(())
}


#[tokio::test]
#[traced_test]
#[serial]
async fn get_block_tx_stream_unordered_correct() -> anyhow::Result<()> {
    let mut client = client().await;
    client.ready().await?;

    let block = client.look_up_block_by_seqno(0, -9223372036854775808, 34716987).await?;

    let len = client.get_block_tx_stream_unordered(&block)
        .count()
        .await;

    assert_eq!(len, 512);

    Ok(())
}
