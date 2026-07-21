use crate::tlb::currency_collection::CurrencyCollection;
use toner::tlb::{Cell, Ref};
use toner_tlb_macros::CellDeserialize;

/// ```tlb
/// _ config_addr:bits256 config:^(Hashmap 32 ^Cell) = ConfigParams;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
pub struct ConfigParams {
    #[tlb(bits)]
    pub config_addr: [u8; 32],
    #[tlb(cell, as = "Ref")]
    pub config: Cell,
}

/// ```tlb
/// masterchain_state_extra#cc26
///   shard_hashes:ShardHashes
///   config:ConfigParams
///   ^[ flags:(## 16) validator_info:ValidatorInfo
///      prev_blocks:OldMcBlocksInfo after_key_block:Bool
///      last_key_block:(Maybe ExtBlkRef)
///      block_create_stats:(flags . 0)?BlockCreateStats ]
///   global_balance:CurrencyCollection
///   = McStateExtra;
/// ```
#[derive(Debug, Clone, CellDeserialize)]
#[tlb(tag = "0xcc26")]
pub struct McStateExtra {
    #[tlb(cell, as = "Option<Ref>")]
    pub shard_hashes: Option<Cell>,
    pub config: ConfigParams,
    #[tlb(cell, as = "Ref")]
    pub state_extra: Cell,
    pub global_balance: CurrencyCollection,
}

#[cfg(test)]
mod tests {
    use super::McStateExtra;
    use num_bigint::BigUint;
    use toner::tlb::bits::NBits;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::vec::BitVec;
    use toner::tlb::bits::ser::BitWriter;
    use toner::tlb::bits::ser::BitWriterExt;
    use toner::tlb::ser::{CellBuilder, CellBuilderError, CellSerialize, CellSerializeExt};
    use toner::tlb::{Cell, Ref};

    #[test]
    fn should_parse_mc_state_extra() {
        let config = Cell::builder().into_cell();
        let state_extra = Cell::builder().into_cell();
        let cell = TestMcStateExtra {
            config: config.clone(),
            state_extra: state_extra.clone(),
        }
        .to_cell(())
        .unwrap();

        let actual: McStateExtra = cell.parse_fully(()).unwrap();

        assert!(actual.shard_hashes.is_none());
        assert_eq!(actual.config.config_addr, [0x11; 32]);
        assert_eq!(actual.config.config, config);
        assert_eq!(actual.state_extra, state_extra);
        assert_eq!(actual.global_balance.grams, BigUint::ZERO);
        assert!(actual.global_balance.other.0.is_empty());
    }

    struct TestMcStateExtra {
        config: Cell,
        state_extra: Cell,
    }

    impl CellSerialize for TestMcStateExtra {
        type Args = ();

        fn store(
            &self,
            builder: &mut CellBuilder,
            _args: Self::Args,
        ) -> Result<(), CellBuilderError> {
            builder
                .pack_as::<_, NBits<16>>(0xcc26_u16, ())?
                .pack(false, ())?;
            builder.write_bitslice(&BitVec::<u8, Msb0>::from_vec(vec![0x11; 32]))?;
            builder
                .store_as::<_, Ref>(&self.config, ())?
                .store_as::<_, Ref>(&self.state_extra, ())?
                .pack_as::<_, NBits<4>>(0_u8, ())?
                .pack(false, ())?;
            Ok(())
        }
    }
}
