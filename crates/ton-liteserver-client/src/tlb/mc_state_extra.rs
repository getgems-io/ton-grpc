use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::ext_blk_ref::ExtBlkRef;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize as CellDeserializeTrait, CellParser, CellParserError};
use toner::tlb::{Cell, Ref};
use toner_tlb_macros::{BitUnpack, CellDeserialize};

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
/// validator_info$_ validator_list_hash_short:uint32
///   catchain_seqno:uint32 nx_cc_updated:Bool = ValidatorInfo;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, BitUnpack)]
pub struct ValidatorInfo {
    pub validator_list_hash_short: u32,
    pub catchain_seqno: u32,
    pub nx_cc_updated: bool,
}

/// ```tlb
/// _ key:Bool max_end_lt:uint64 = KeyMaxLt;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, BitUnpack)]
pub struct KeyMaxLt {
    pub key: bool,
    pub max_end_lt: u64,
}

/// ```tlb
/// _ key:Bool blk_ref:ExtBlkRef = KeyExtBlkRef;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, BitUnpack)]
pub struct KeyExtBlkRef {
    pub key: bool,
    pub blk_ref: ExtBlkRef,
}

/// Lazy outer representation of `(HashmapAugE 32 KeyExtBlkRef KeyMaxLt)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldMcBlocksInfo {
    pub root: Option<Cell>,
    pub extra: KeyMaxLt,
}

impl<'de> CellDeserializeTrait<'de> for OldMcBlocksInfo {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let root = if parser.unpack(())? {
            Some(parser.parse_as::<_, Ref>(())?)
        } else {
            None
        };

        Ok(Self {
            root,
            extra: parser.unpack(())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockCreateStats {
    Ordinary { root: Option<Cell> },
    Extended { root: Option<Cell>, extra: u32 },
}

impl<'de> CellDeserializeTrait<'de> for BlockCreateStats {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u8 = parser.unpack_as::<_, toner::tlb::bits::NBits<8>>(())?;
        let root = if parser.unpack(())? {
            Some(parser.parse_as::<_, Ref>(())?)
        } else {
            None
        };

        match tag {
            0x17 => Ok(Self::Ordinary { root }),
            0x34 => Ok(Self::Extended {
                root,
                extra: parser.unpack(())?,
            }),
            _ => Err(toner::tlb::Error::custom(format!(
                "invalid BlockCreateStats tag: 0x{tag:02x}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McStateExtraInfo {
    pub flags: u16,
    pub validator_info: ValidatorInfo,
    pub prev_blocks: OldMcBlocksInfo,
    pub after_key_block: bool,
    pub last_key_block: Option<ExtBlkRef>,
    pub block_create_stats: Option<BlockCreateStats>,
}

impl<'de> CellDeserializeTrait<'de> for McStateExtraInfo {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let flags: u16 = parser.unpack_as::<_, toner::tlb::bits::NBits<16>>(())?;
        if flags > 1 {
            return Err(toner::tlb::Error::custom(format!(
                "unsupported McStateExtraInfo flags: {flags}"
            )));
        }

        let validator_info = parser.unpack(())?;
        let prev_blocks = parser.parse(())?;
        let after_key_block = parser.unpack(())?;
        let last_key_block = if parser.unpack(())? {
            Some(parser.unpack(())?)
        } else {
            None
        };
        let block_create_stats = if flags & 1 != 0 {
            Some(parser.parse(())?)
        } else {
            None
        };

        Ok(Self {
            flags,
            validator_info,
            prev_blocks,
            after_key_block,
            last_key_block,
            block_create_stats,
        })
    }
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
    pub state_extra: McStateExtraInfo,
    pub global_balance: CurrencyCollection,
}

#[cfg(test)]
mod tests {
    use super::{BlockCreateStats, McStateExtra, McStateExtraInfo};
    use num_bigint::BigUint;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::vec::BitVec;
    use toner::tlb::bits::ser::BitWriter;
    use toner::tlb::bits::ser::BitWriterExt;
    use toner::tlb::ser::{CellBuilder, CellBuilderError, CellSerialize, CellSerializeExt};
    use toner::tlb::{Cell, Ref};

    #[test]
    fn should_parse_mc_state_extra() {
        let config = Cell::builder().into_cell();
        let state_extra = TestMcStateExtraInfo {
            flags: 0,
            block_create_stats: None,
        }
        .to_cell(())
        .unwrap();
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
        assert_eq!(actual.state_extra.flags, 0);
        assert_eq!(actual.state_extra.validator_info.catchain_seqno, 2);
        assert!(actual.state_extra.prev_blocks.root.is_none());
        assert!(!actual.state_extra.after_key_block);
        assert!(actual.state_extra.last_key_block.is_none());
        assert!(actual.state_extra.block_create_stats.is_none());
        assert_eq!(actual.global_balance.grams, BigUint::ZERO);
        assert!(actual.global_balance.other.0.is_empty());
    }

    #[test]
    fn should_parse_extended_block_create_stats() {
        let root = Cell::builder().into_cell();
        let cell = TestMcStateExtraInfo {
            flags: 1,
            block_create_stats: Some(TestBlockCreateStats::Extended {
                root: root.clone(),
                extra: 42,
            }),
        }
        .to_cell(())
        .unwrap();

        let actual: McStateExtraInfo = cell.parse_fully(()).unwrap();

        assert!(matches!(
            actual.block_create_stats,
            Some(BlockCreateStats::Extended {
                root: Some(actual_root),
                extra: 42,
            }) if actual_root == root
        ));
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
                .pack_as::<_, toner::tlb::bits::NBits<16>>(0xcc26_u16, ())?
                .pack(false, ())?;
            builder.write_bitslice(&BitVec::<u8, Msb0>::from_vec(vec![0x11; 32]))?;
            builder
                .store_as::<_, Ref>(&self.config, ())?
                .store_as::<_, Ref>(&self.state_extra, ())?
                .pack_as::<_, toner::tlb::bits::NBits<4>>(0_u8, ())?
                .pack(false, ())?;
            Ok(())
        }
    }

    struct TestMcStateExtraInfo {
        flags: u16,
        block_create_stats: Option<TestBlockCreateStats>,
    }

    enum TestBlockCreateStats {
        Extended { root: Cell, extra: u32 },
    }

    impl CellSerialize for TestMcStateExtraInfo {
        type Args = ();

        fn store(
            &self,
            builder: &mut CellBuilder,
            _args: Self::Args,
        ) -> Result<(), CellBuilderError> {
            builder
                .pack_as::<_, toner::tlb::bits::NBits<16>>(self.flags, ())?
                .pack(1_u32, ())?
                .pack(2_u32, ())?
                .pack(false, ())?
                .pack(false, ())?
                .pack(false, ())?
                .pack(0_u64, ())?
                .pack(false, ())?
                .pack(false, ())?;
            if let Some(TestBlockCreateStats::Extended { root, extra }) = &self.block_create_stats {
                builder
                    .pack(0x34_u8, ())?
                    .pack(true, ())?
                    .store_as::<_, Ref>(root, ())?
                    .pack(*extra, ())?;
            }
            Ok(())
        }
    }
}
