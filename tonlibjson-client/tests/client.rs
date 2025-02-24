use futures::StreamExt;
use tonlibjson_client::block::{InternalTransactionId, RawTransaction};
use tonlibjson_client::ton::{TonClient, TonClientBuilder};
use tracing::debug;
use tracing_test::traced_test;

async fn client() -> TonClient {
    let mut client = TonClientBuilder::default().build().unwrap();
    client.ready().await.unwrap();

    client
}

#[tokio::test]
#[traced_test]
#[ignore]
async fn get_account_tx_stream_starts_from() -> anyhow::Result<()> {
    let client = client().await;
    let address = "EQCjk1hh952vWaE9bRguFkAhDAL5jj3xj9p0uPWrFBq_GEMS".to_owned();
    let hash = "752Szayka+Eh54Zvco5l84d6WL+zJFmyh1wqRxD08Uo=";
    let lt = 33756943000007;
    let tx = InternalTransactionId {
        hash: hash.to_owned(),
        lt: lt.to_owned(),
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
#[ignore]
async fn get_account_tx_stream_contains_only_one_transaction() -> anyhow::Result<()> {
    let client = client().await;
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
#[ignore]
async fn get_block_tx_stream_correct() -> anyhow::Result<()> {
    let client = client().await;
    let block = client
        .look_up_block_by_seqno(0, -9223372036854775808, 34716987)
        .await?;

    let len = client.get_block_tx_id_stream(&block, false).count().await;

    assert_eq!(len, 512);
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
async fn get_block_tx_stream_reverse_correct() -> anyhow::Result<()> {
    let client = client().await;
    let block = client
        .look_up_block_by_seqno(0, -9223372036854775808, 34716987)
        .await?;

    let len = client.get_block_tx_id_stream(&block, true).count().await;

    assert_eq!(len, 512);
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
async fn get_block_tx_stream_unordered_correct() -> anyhow::Result<()> {
    let client = client().await;
    let block = client
        .look_up_block_by_seqno(0, -9223372036854775808, 34716987)
        .await?;

    let len = client.get_block_tx_stream_unordered(&block).count().await;

    assert_eq!(len, 512);
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
async fn get_block_header_no_hashes() -> anyhow::Result<()> {
    let client = client().await;
    let mc_block = client.get_masterchain_info().await?;

    let mc_header = client
        .get_block_header(
            mc_block.last.workchain,
            mc_block.last.shard,
            mc_block.last.seqno,
            None,
        )
        .await?;

    assert_eq!(mc_header.id.seqno, mc_block.last.seqno);
    assert!(!mc_header.want_split);
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
async fn get_block_header_with_hashes() -> anyhow::Result<()> {
    let client = client().await;
    let mc_block = client.get_masterchain_info().await?;

    let mc_header = client
        .get_block_header(
            mc_block.last.workchain,
            mc_block.last.shard,
            mc_block.last.seqno,
            Some((mc_block.last.root_hash, mc_block.last.file_hash)),
        )
        .await?;

    assert_eq!(mc_header.id.seqno, mc_block.last.seqno);
    assert!(!mc_header.want_split);
    Ok(())
}
