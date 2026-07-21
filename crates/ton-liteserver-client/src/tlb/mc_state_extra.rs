use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::ext_blk_ref::ExtBlkRef;
use crate::tlb::hashmap::{Hashmap, HashmapAugE, HashmapLookup};
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize as CellDeserializeTrait, CellParser, CellParserError};
use toner::tlb::{Cell, Data, Ref, Same, StringError};
use toner_tlb_macros::{BitPack, BitUnpack, CellDeserialize};

/// ```tlb
/// _ config_addr:bits256 config:^(Hashmap 32 ^Cell) = ConfigParams;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
pub struct ConfigParams {
    #[tlb(bits)]
    pub config_addr: [u8; 32],
    #[tlb(cell, as = "Ref<Hashmap<u32, Ref, 32, Same>>", args = "((), ())")]
    pub config: Hashmap<u32, Cell, 32>,
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
#[derive(Debug, Clone, PartialEq, Eq, BitPack, BitUnpack)]
pub struct KeyMaxLt {
    pub key: bool,
    pub max_end_lt: u64,
}

/// ```tlb
/// _ key:Bool blk_ref:ExtBlkRef = KeyExtBlkRef;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, BitPack, BitUnpack)]
pub struct KeyExtBlkRef {
    pub key: bool,
    pub blk_ref: ExtBlkRef,
}

#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
pub struct OldMcBlocksInfo(
    #[tlb(cell, as = "HashmapAugE<u32, Data, 32, Data>", args = "((), ())")]
    pub  HashmapAugE<u32, KeyExtBlkRef, 32, KeyMaxLt>,
);

impl OldMcBlocksInfo {
    pub fn lookup(&self, seqno: u32) -> Result<HashmapLookup<'_, KeyExtBlkRef>, StringError> {
        let result = self.0.hashmap.lookup(&seqno);
        if let HashmapLookup::Found(value) = result
            && value.blk_ref.seq_no != seqno
        {
            return Err(toner::tlb::Error::custom(format!(
                "old masterchain block seqno mismatch: expected {seqno}, got {}",
                value.blk_ref.seq_no
            )));
        }
        Ok(result)
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
    use super::{BlockCreateStats, KeyMaxLt, McStateExtra, McStateExtraInfo, OldMcBlocksInfo};
    use crate::tlb::ext_blk_ref::ExtBlkRef;
    use crate::tlb::hashmap::{
        Hashmap, HashmapAugE, HashmapAugNode, HashmapE, HashmapLookup, HashmapNode, HashmapTree,
    };
    use num_bigint::BigUint;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::vec::BitVec;
    use toner::tlb::bits::ser::BitWriter;
    use toner::tlb::bits::ser::BitWriterExt;
    use toner::tlb::ser::{CellBuilder, CellBuilderError, CellSerialize, CellSerializeExt};
    use toner::tlb::{Cell, Ref};

    #[test]
    fn should_parse_mc_state_extra() {
        let config = pruned_cell();
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
        assert!(matches!(
            &actual.config.config.tree,
            HashmapTree::Pruned(actual) if actual == &config
        ));
        assert_eq!(actual.state_extra.flags, 0);
        assert_eq!(actual.state_extra.validator_info.catchain_seqno, 2);
        assert!(actual.state_extra.prev_blocks.0.hashmap.is_empty());
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

    #[test]
    fn should_lookup_old_mc_block() {
        let block = old_mc_block(0);
        let old_blocks = old_mc_blocks(Hashmap::new(HashmapTree::Edge {
            prefix: BitVec::repeat(false, 32),
            node: HashmapAugNode {
                extra: key_max_lt(),
                node: HashmapNode::Leaf(block.clone()),
            },
        }));

        let actual = old_blocks.lookup(0).unwrap();

        assert_eq!(actual, HashmapLookup::Found(&block));
    }

    #[test]
    fn should_report_absent_old_mc_block() {
        let old_blocks = old_mc_blocks(Hashmap::new(HashmapTree::Edge {
            prefix: BitVec::repeat(false, 32),
            node: HashmapAugNode {
                extra: key_max_lt(),
                node: HashmapNode::Leaf(old_mc_block(0)),
            },
        }));

        let actual = old_blocks.lookup(1).unwrap();

        assert_eq!(actual, HashmapLookup::Absent);
    }

    #[test]
    fn should_report_pruned_old_mc_block() {
        let pruned = pruned_cell();
        let old_blocks = old_mc_blocks(Hashmap::new(HashmapTree::Pruned(pruned.clone())));

        let actual = old_blocks.lookup(0).unwrap();

        assert_eq!(actual, HashmapLookup::Pruned(&pruned));
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

    fn old_mc_block(seq_no: u32) -> super::KeyExtBlkRef {
        super::KeyExtBlkRef {
            key: false,
            blk_ref: ExtBlkRef {
                end_lt: 100,
                seq_no,
                root_hash: [1; 32],
                file_hash: [2; 32],
            },
        }
    }

    fn old_mc_blocks(root: Hashmap<u32, super::KeyExtBlkRef, 32, KeyMaxLt>) -> OldMcBlocksInfo {
        OldMcBlocksInfo(HashmapAugE {
            hashmap: HashmapE::Root(root),
            extra: key_max_lt(),
        })
    }

    fn key_max_lt() -> KeyMaxLt {
        KeyMaxLt {
            key: false,
            max_end_lt: 0,
        }
    }

    fn pruned_cell() -> Cell {
        let mut data = vec![0_u8; 36];
        data[0] = 1;
        data[1] = 1;
        Cell {
            is_exotic: true,
            data: BitVec::from_vec(data),
            references: Vec::new(),
        }
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
