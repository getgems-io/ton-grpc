use crate::tlb::ext_blk_ref::ExtBlkRef;
use toner_tlb_macros::BitUnpack;

/// ```tlb
/// master_info$_ master:ExtBlkRef = BlkMasterInfo;
/// ```
#[derive(Debug, Clone, BitUnpack)]
pub struct BlkMasterInfo {
    pub master: ExtBlkRef,
}
