use tonlibjson_client::block;

tonic::include_proto!("ton");

impl From<block::BlockIdExt> for BlockIdExt {
    fn from(o: block::BlockIdExt) -> Self {
        Self {
            workchain: o.workchain,
            shard: o.shard,
            seqno: o.seqno,
            root_hash: o.root_hash,
            file_hash: o.file_hash
        }
    }
}

impl From<block::MasterchainInfo> for GetMasterchainInfoResponse {
    fn from(o: block::MasterchainInfo) -> Self {
        Self {
            init: Some(o.init.into()),
            last: Some(o.last.into()),
            state_root_hash: o.state_root_hash
        }
    }
}
