use toner_tlb_macros::BitUnpack;

/// ```tlb
/// fsm_none$0 = FutureSplitMerge;
/// fsm_split$10 split_utime:uint32 interval:uint32 = FutureSplitMerge;
/// fsm_merge$11 merge_utime:uint32 interval:uint32 = FutureSplitMerge;
/// ```
#[derive(Debug, Clone, BitUnpack)]
pub enum FutureSplitMerge {
    #[tlb(tag = "0b0")]
    None,
    #[tlb(tag = "0b10")]
    Split { split_utime: u32, interval: u32 },
    #[tlb(tag = "0b11")]
    Merge { merge_utime: u32, interval: u32 },
}
