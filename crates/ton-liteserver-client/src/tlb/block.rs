use crate::tlb::block_extra::BlockExtra;
use crate::tlb::block_info::BlockInfo;
use crate::tlb::merkle_update::MerkleUpdate;
use toner::tlb::{Cell, ParseFully, Ref};
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// block#11ef55aa global_id:int32
///   info:^BlockInfo value_flow:^ValueFlow
///   state_update:^(MERKLE_UPDATE ShardState)
///   extra:^BlockExtra = Block;
/// ```
#[derive(Debug, Clone, CellDeserialize)]
#[tlb(tag = "0x11ef55aa")]
pub struct Block {
    #[tlb(bits)]
    pub global_id: i32,
    #[tlb(cell, as = "Ref<ParseFully>")]
    pub info: BlockInfo,
    #[tlb(cell, as = "Ref<ParseFully>")]
    pub value_flow: Cell,
    #[tlb(cell, as = "Ref<ParseFully>")]
    pub state_update: MerkleUpdate<Cell>,
    #[tlb(cell, as = "Ref<ParseFully>")]
    pub extra: BlockExtra,
}

#[cfg(test)]
mod tests {
    use crate::tlb::block::Block;
    use crate::tlb::tests::BLOCK_HEX;
    use std::sync::Arc;
    use toner::tlb::bits::de::unpack_bytes;
    use toner::tlb::{BoC, Cell};

    #[test]
    fn test_block_parse_ok() {
        let root = given_block_root_cell();

        let block: Block = root.parse_fully(()).unwrap();

        assert_eq!(block.global_id, -239);
    }

    fn given_block_root_cell() -> Arc<Cell> {
        let data = hex::decode(BLOCK_HEX).unwrap();

        unpack_bytes::<BoC>(&data, ())
            .unwrap()
            .into_single_root()
            .unwrap()
    }
}
