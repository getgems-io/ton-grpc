#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockCriteria {
    Seqno { shard: i64, seqno: i32 },
    LogicalTime(i64)
}

#[derive(Debug, Clone, Copy)]
pub enum Route {
    Block { chain: i32, criteria: BlockCriteria },
    Latest
}
