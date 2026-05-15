use toner_tlb_macros::BitUnpack;

/// ```tlb
/// capabilities#c4 version:uint32 capabilities:uint64 = GlobalVersion;
/// ```
#[derive(Debug, Clone, BitUnpack)]
#[tlb(tag = "0xc4")]
pub struct GlobalVersion {
    pub version: u32,
    pub capabilities: u64,
}
