use serde::Serialize;
use serde_json::Value;
use tonlibjson_client::block::{InternalTransactionId, RawMessage, RawTransaction};

#[derive(Serialize)]
pub struct MessageView {
    pub source: String,
    pub destination: String,
    pub value: String,
    pub fwd_fee: String,
    pub ihr_fee: String,
    pub created_lt: String,
    pub body_hash: String,
    pub msg_data: Value
}

impl From<&RawMessage> for MessageView {
    fn from(msg: &RawMessage) -> Self {
        Self {
            source: msg.source.account_address.clone(),
            destination: msg.destination.account_address.clone(),
            value: msg.value.to_string(),
            fwd_fee: msg.fwd_fee.clone(),
            ihr_fee: msg.ihr_fee.clone(),
            created_lt: msg.created_lt.clone(),
            body_hash: msg.body_hash.clone(),
            msg_data: msg.msg_data.clone()
        }
    }
}

#[derive(Serialize)]
pub struct TransactionView {
    pub utime: i64,
    pub data: String,
    pub transaction_id: InternalTransactionId,
    pub fee: String,
    pub storage_fee: String,
    pub other_fee: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_msg: Option<MessageView>,
    pub out_msgs: Vec<MessageView>
}

impl From<&RawTransaction> for TransactionView {
    fn from(tx: &RawTransaction) -> Self {
        Self {
            utime: tx.utime,
            data: tx.data.clone(),
            transaction_id: tx.transaction_id.clone(),
            fee: tx.fee.to_string(),
            storage_fee: tx.storage_fee.to_string(),
            other_fee: tx.other_fee.to_string(),
            in_msg: tx.in_msg.as_ref().map(|msg| msg.into()),
            out_msgs: tx.out_msgs.iter().map(Into::into).collect(),
        }
    }
}
