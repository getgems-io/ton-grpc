use crate::tl::{
    BoxedBool, LiteServerBlockTransactions, LiteServerMasterchainInfo, LiteServerTransactionId3,
    TonNodeBlockId, TonNodeBlockIdExt,
};
use crate::tlb::blk_prev_info::BlkPrevInfo;
use crate::tlb::block_header::BlockHeader;
use crate::tlb::ext_blk_ref::ExtBlkRef;
use crate::tlb::shard_ident::ShardIdent;
use crate::tlb::transaction::Transaction;
use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as base64_standard;
use std::sync::Arc;
use ton_address::SmartContractAddress;
use ton_client::ShortTxId;
use toner::tlb::{BagOfCellsArgs, BoC, Cell};
use toner::ton::message::{CommonMsgInfo, InternalMsgInfo, Message};

impl From<TonNodeBlockIdExt> for ton_client::BlockIdExt {
    fn from(v: TonNodeBlockIdExt) -> Self {
        Self {
            workchain: v.workchain,
            shard: v.shard,
            seqno: v.seqno,
            root_hash: base64_standard.encode(v.root_hash),
            file_hash: base64_standard.encode(v.file_hash),
        }
    }
}

impl From<ton_client::BlockIdExt> for TonNodeBlockIdExt {
    fn from(v: ton_client::BlockIdExt) -> Self {
        Self {
            workchain: v.workchain,
            shard: v.shard,
            seqno: v.seqno,
            root_hash: base64_standard
                .decode(&v.root_hash)
                .expect("valid base64 root_hash")
                .try_into()
                .expect("root_hash must be 32 bytes"),
            file_hash: base64_standard
                .decode(&v.file_hash)
                .expect("valid base64 file_hash")
                .try_into()
                .expect("file_hash must be 32 bytes"),
        }
    }
}

impl From<ton_client::BlockIdExt> for TonNodeBlockId {
    fn from(v: ton_client::BlockIdExt) -> Self {
        Self {
            workchain: v.workchain,
            shard: v.shard,
            seqno: v.seqno,
        }
    }
}

impl From<LiteServerMasterchainInfo> for ton_client::MasterchainInfo {
    fn from(v: LiteServerMasterchainInfo) -> Self {
        Self {
            last: v.last.into(),
            state_root_hash: base64_standard.encode(v.state_root_hash),
            init: ton_client::BlockIdExt {
                workchain: v.init.workchain,
                shard: 0,
                seqno: 0,
                root_hash: base64_standard.encode(v.init.root_hash),
                file_hash: base64_standard.encode(v.init.file_hash),
            },
        }
    }
}

fn ext_blk_ref_to_block_id_ext(shard: &ShardIdent, r: &ExtBlkRef) -> ton_client::BlockIdExt {
    ton_client::BlockIdExt {
        workchain: shard.workchain_id,
        shard: shard.shard_prefix as i64,
        seqno: r.seq_no as i32,
        root_hash: base64_standard.encode(r.root_hash),
        file_hash: base64_standard.encode(r.file_hash),
    }
}

fn blk_prev_info_to_block_ids(
    shard: &ShardIdent,
    prev: &BlkPrevInfo,
) -> Vec<ton_client::BlockIdExt> {
    match prev {
        BlkPrevInfo::Ref(r) => vec![ext_blk_ref_to_block_id_ext(shard, r)],
        BlkPrevInfo::RefPair(a, b) => vec![
            ext_blk_ref_to_block_id_ext(shard, a),
            ext_blk_ref_to_block_id_ext(shard, b),
        ],
    }
}

pub fn block_header_to_ton_client(
    id: ton_client::BlockIdExt,
    header: BlockHeader,
) -> ton_client::BlockHeader {
    let info = &header.info;
    let flags = info.flags;

    ton_client::BlockHeader {
        id,
        global_id: header.global_id,
        version: info.version as i32,
        flags: flags as i32,
        after_merge: flags & (1 << 14) != 0,
        after_split: flags & (1 << 12) != 0,
        before_split: flags & (1 << 13) != 0,
        want_merge: flags & (1 << 10) != 0,
        want_split: flags & (1 << 11) != 0,
        validator_list_hash_short: info.gen_validator_list_hash_short as i32,
        catchain_seqno: info.gen_catchain_seqno as i32,
        min_ref_mc_seqno: info.min_ref_mc_seqno as i32,
        is_key_block: flags & (1 << 9) != 0,
        prev_key_block_seqno: info.prev_key_block_seqno as i32,
        start_lt: info.start_lt as i64,
        end_lt: info.end_lt as i64,
        gen_utime: info.gen_utime as i64,
        vert_seqno: info.vert_seq_no as i32,
        prev_blocks: blk_prev_info_to_block_ids(&info.shard, &info.prev_ref),
    }
}

pub fn block_transactions_to_ton_client(
    v: LiteServerBlockTransactions,
) -> anyhow::Result<ton_client::BlockTransactions> {
    let workchain = v.id.workchain;
    let incomplete = matches!(v.incomplete, BoxedBool::BoolTrue(_));

    let transactions = v
        .ids
        .into_iter()
        .map(|tx| {
            let account = tx
                .account
                .ok_or_else(|| anyhow!("transaction id missing account"))?;
            let lt = tx.lt.ok_or_else(|| anyhow!("transaction id missing lt"))?;
            let hash = tx
                .hash
                .as_ref()
                .ok_or_else(|| anyhow!("transaction id missing hash"))?;

            Ok(ShortTxId {
                account: SmartContractAddress::raw(workchain, account).to_bounceable(),
                lt,
                hash: base64_standard.encode(hash),
            })
        })
        .collect::<anyhow::Result<Vec<ShortTxId>>>()?;

    Ok(ton_client::BlockTransactions {
        incomplete,
        transactions,
    })
}

impl From<ton_client::ShortTxId> for LiteServerTransactionId3 {
    fn from(v: ton_client::ShortTxId) -> Self {
        Self {
            account: *v.account.to_internal(),
            lt: v.lt,
        }
    }
}

fn tlb_message_to_ton_client(msg: &Message) -> ton_client::Message {
    let (source, destination, value, fwd_fee, ihr_fee, created_lt) = match &msg.info {
        CommonMsgInfo::Internal(InternalMsgInfo {
            src,
            dst,
            value,
            fwd_fee,
            ihr_fee,
            created_lt,
            ..
        }) => {
            let src = msg_address_to_sca(src);
            let dst = msg_address_to_sca(dst);
            let value = biguint_to_i64(&value.grams);
            let fwd_fee = biguint_to_i64(fwd_fee);
            let ihr_fee = biguint_to_i64(ihr_fee);
            (src, dst, value, fwd_fee, ihr_fee, *created_lt as i64)
        }
        CommonMsgInfo::ExternalIn(info) => {
            let dst = msg_address_to_sca(&info.dst);
            (None, dst, 0, 0, 0, 0)
        }
        CommonMsgInfo::ExternalOut(info) => {
            let src = msg_address_to_sca(&info.src);
            (src, None, 0, 0, 0, info.created_lt as i64)
        }
    };

    ton_client::Message {
        hash: String::new(),
        source,
        destination,
        value,
        fwd_fee,
        ihr_fee,
        created_lt,
        body_hash: String::new(),
        msg_data: ton_client::MessageData::Raw {
            body: String::new(),
            init_state: String::new(),
        },
    }
}

fn msg_address_to_sca(addr: &toner::ton::MsgAddress) -> Option<SmartContractAddress> {
    if addr.address == [0u8; 32] && addr.workchain_id == 0 {
        return None;
    }
    Some(SmartContractAddress::raw(addr.workchain_id, addr.address).to_bounceable())
}

fn biguint_to_i64(v: &num_bigint::BigUint) -> i64 {
    v.try_into().unwrap_or(i64::MAX)
}

fn extract_total_fees(tx: &Transaction) -> i64 {
    biguint_to_i64(&tx.total_fees.grams)
}

pub fn transaction_to_ton_client(
    workchain: i32,
    root: &Arc<Cell>,
    tx: Transaction,
) -> anyhow::Result<ton_client::Transaction> {
    let data = {
        let boc = BoC::from_root(root.clone());
        let bytes = boc
            .serialize(BagOfCellsArgs {
                has_crc32c: true,
                ..BagOfCellsArgs::default()
            })
            .map_err(|e| anyhow!("{e}"))?;
        base64_standard.encode(&bytes)
    };

    let address = SmartContractAddress::raw(workchain, tx.account_addr).to_bounceable();
    let fee = extract_total_fees(&tx);

    let in_msg = tx.in_msg.as_ref().map(tlb_message_to_ton_client);

    let mut out_msgs: Vec<_> = tx.out_msgs.iter().collect();
    out_msgs.sort_by_key(|(k, _)| *k);
    let out_msgs: Vec<ton_client::Message> = out_msgs
        .into_iter()
        .map(|(_, msg)| tlb_message_to_ton_client(msg))
        .collect();

    Ok(ton_client::Transaction {
        address,
        utime: tx.now as i64,
        data,
        transaction_id: ton_client::TransactionId {
            lt: tx.lt as i64,
            hash: base64_standard.encode(root.hash()),
        },
        fee,
        storage_fee: 0,
        other_fee: 0,
        in_msg,
        out_msgs,
    })
}
