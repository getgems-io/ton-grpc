use futures::StreamExt;
use testcontainers_ton::LocalLiteServer;
use tokio::net::TcpListener;
use ton_grpc::account_service_server::AccountServiceServer;
use ton_grpc::block_service_server::BlockServiceServer;
use ton_grpc::ton::account_service_client::AccountServiceClient;
use ton_grpc::ton::block_service_client::BlockServiceClient;
use ton_grpc::ton::{
    BlockId, GetAccountStateRequest, GetAccountTransactionsRequest, GetLastBlockRequest,
    GetShardAccountCellRequest, GetTransactionsRequest, get_account_state_request,
    get_shard_account_cell_request,
};
use ton_grpc::{AccountService, BlockService};
use tonic::transport::Channel;
use tonlibjson_client::ton::TonClientBuilder;
use tracing_test::traced_test;

#[tokio::test]
#[traced_test]
async fn should_chain_block_transactions_to_account_state() {
    let (_server, mut blocks, mut accounts) = setup().await;

    let last = blocks
        .get_last_block(GetLastBlockRequest {})
        .await
        .unwrap()
        .into_inner();
    tracing::info!("last block: {:?}", last);
    assert_eq!(last.workchain, -1);
    assert!(last.seqno > 0);

    let block_id = BlockId {
        workchain: last.workchain,
        shard: last.shard,
        seqno: last.seqno,
        root_hash: None,
        file_hash: None,
    };
    let tx_stream = blocks
        .get_transactions(GetTransactionsRequest {
            block_id: Some(block_id),
            order: ton_grpc::ton::get_transactions_request::Order::Asc as i32,
        })
        .await
        .unwrap()
        .into_inner();
    let txs: Vec<_> = tx_stream
        .take(10)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    tracing::info!("transactions: {:?}", txs);
    assert!(!txs.is_empty());

    let first_tx = &txs[0];
    let tx_id = first_tx.id.as_ref().unwrap();
    assert!(!tx_id.account_address.is_empty());
    assert!(tx_id.lt > 0);
    assert_eq!(tx_id.hash.len(), 44);
    assert!(first_tx.utime > 0);

    let account_address = tx_id.account_address.clone();
    let account_state = accounts
        .get_account_state(GetAccountStateRequest {
            account_address: account_address.clone(),
            criteria: Some(get_account_state_request::Criteria::BlockId(BlockId {
                workchain: last.workchain,
                shard: last.shard,
                seqno: last.seqno,
                root_hash: Some(last.root_hash.clone()),
                file_hash: Some(last.file_hash.clone()),
            })),
        })
        .await
        .unwrap()
        .into_inner();
    tracing::info!("account state: {:?}", account_state);
    assert_eq!(account_state.account_address, account_address);
    assert!(account_state.block_id.is_some());
    assert!(account_state.account_state.is_some());

    let last_tx = account_state.last_transaction_id.as_ref().unwrap();
    assert!(last_tx.lt > 0);
    assert_eq!(last_tx.hash.len(), 44);

    let account_tx_stream = accounts
        .get_account_transactions(GetAccountTransactionsRequest {
            account_address: account_address.clone(),
            order: ton_grpc::ton::get_account_transactions_request::Order::FromNewToOld as i32,
            from: None,
            to: None,
        })
        .await
        .unwrap()
        .into_inner();
    let account_txs: Vec<_> = account_tx_stream
        .take(5)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    tracing::info!("account transactions: {:?}", account_txs);
    assert!(!account_txs.is_empty());

    let found = account_txs
        .iter()
        .any(|tx| tx.id.as_ref().unwrap().hash == tx_id.hash);
    assert!(
        found,
        "transaction from block must appear in account transaction history"
    );

    for tx in &account_txs {
        let id = tx.id.as_ref().unwrap();
        assert_eq!(id.account_address, account_address);
        assert!(id.lt > 0);
        assert_eq!(id.hash.len(), 44);
        assert!(tx.utime > 0);
        assert!(!tx.data.is_empty());
    }
}

#[tokio::test]
#[traced_test]
async fn should_chain_block_header_to_shard_transactions_to_account_cell() {
    let (_server, mut blocks, mut accounts) = setup().await;

    let last = blocks
        .get_last_block(GetLastBlockRequest {})
        .await
        .unwrap()
        .into_inner();
    tracing::info!("last block: {:?}", last);

    let header = blocks
        .get_block_header(BlockId {
            workchain: last.workchain,
            shard: last.shard,
            seqno: last.seqno,
            root_hash: None,
            file_hash: None,
        })
        .await
        .unwrap()
        .into_inner();
    tracing::info!("header: {:?}", header);
    let header_id = header.id.as_ref().unwrap();
    assert_eq!(header_id.workchain, -1);
    assert_eq!(header_id.seqno, last.seqno);
    assert!(header.end_lt >= header.start_lt);
    assert!(header.gen_utime > 0);

    let block_id = BlockId {
        workchain: last.workchain,
        shard: last.shard,
        seqno: last.seqno,
        root_hash: None,
        file_hash: None,
    };

    let addr_stream = blocks
        .get_account_addresses(block_id)
        .await
        .unwrap()
        .into_inner();
    let addresses: Vec<_> = addr_stream
        .take(5)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    tracing::info!("addresses: {:?}", addresses);
    assert!(!addresses.is_empty());

    let address = &addresses[0].address;
    assert!(!address.is_empty());

    let account_state = accounts
        .get_account_state(GetAccountStateRequest {
            account_address: address.clone(),
            criteria: Some(get_account_state_request::Criteria::BlockId(BlockId {
                workchain: last.workchain,
                shard: last.shard,
                seqno: last.seqno,
                root_hash: Some(last.root_hash.clone()),
                file_hash: Some(last.file_hash.clone()),
            })),
        })
        .await
        .unwrap()
        .into_inner();
    tracing::info!("account state: {:?}", account_state);
    assert_eq!(account_state.account_address, *address);
    assert!(account_state.block_id.is_some());
    assert!(account_state.account_state.is_some());

    let cell = accounts
        .get_shard_account_cell(GetShardAccountCellRequest {
            account_address: address.clone(),
            criteria: Some(get_shard_account_cell_request::Criteria::BlockId(BlockId {
                workchain: last.workchain,
                shard: last.shard,
                seqno: last.seqno,
                root_hash: Some(last.root_hash.clone()),
                file_hash: Some(last.file_hash.clone()),
            })),
        })
        .await
        .unwrap()
        .into_inner();
    tracing::info!("cell: {:?}", cell);
    assert_eq!(cell.account_address, *address);
    assert!(cell.block_id.is_some());
    assert!(!cell.cell.unwrap().bytes.is_empty());
}

async fn setup() -> (
    LocalLiteServer,
    BlockServiceClient<Channel>,
    AccountServiceClient<Channel>,
) {
    let server = LocalLiteServer::new().await.unwrap();
    let mut client = TonClientBuilder::from_config(server.config())
        .build()
        .unwrap();
    client.ready().await.unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(BlockServiceServer::new(BlockService::new(client.clone())))
            .add_service(AccountServiceServer::new(AccountService::new(client)))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    let channel = Channel::from_shared(format!("http://{}", addr))
        .unwrap()
        .connect()
        .await
        .unwrap();

    (
        server,
        BlockServiceClient::new(channel.clone()),
        AccountServiceClient::new(channel),
    )
}
