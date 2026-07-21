use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::ext_blk_ref::ExtBlkRef;
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::slice::BitSlice;
use toner::tlb::bits::bitvec::vec::BitVec;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize as CellDeserializeTrait, CellParser, CellParserError};
use toner::tlb::{Cell, Ref, StringError};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OldMcBlockLookup {
    Found(KeyExtBlkRef),
    Absent,
    Pruned,
}

pub(crate) enum CellLookup {
    Found(Cell),
    Absent,
    Pruned,
}

pub(crate) fn lookup_cell_hashmap(root: &Cell, key: u32) -> Result<CellLookup, StringError> {
    let key = BitVec::<u8, Msb0>::from_vec(key.to_be_bytes().to_vec());

    lookup_cell_hashmap_edge(root, key.as_bitslice(), 32)
}

impl OldMcBlocksInfo {
    pub fn lookup(&self, seqno: u32) -> Result<OldMcBlockLookup, StringError> {
        let Some(root) = &self.root else {
            return Ok(OldMcBlockLookup::Absent);
        };
        let key = BitVec::<u8, Msb0>::from_vec(seqno.to_be_bytes().to_vec());

        lookup_old_mc_block(root, key.as_bitslice(), 32, seqno)
    }
}

fn lookup_old_mc_block(
    cell: &Cell,
    key: &BitSlice<u8, Msb0>,
    n: u32,
    seqno: u32,
) -> Result<OldMcBlockLookup, StringError> {
    if cell.is_exotic {
        return if cell.data.as_raw_slice().first() == Some(&1) {
            Ok(OldMcBlockLookup::Pruned)
        } else {
            Err(toner::tlb::Error::custom("unexpected exotic hashmap cell"))
        };
    }

    let mut parser = cell.parser();
    let label = parse_hm_label(&mut parser, n)?;
    if !key.starts_with(&label) {
        return Ok(OldMcBlockLookup::Absent);
    }
    let m = n
        .checked_sub(label.len() as u32)
        .ok_or_else(|| toner::tlb::Error::custom("hashmap label exceeds remaining key"))?;
    let remaining = &key[label.len()..];
    let _: KeyMaxLt = parser.unpack(())?;

    if m == 0 {
        let value: KeyExtBlkRef = parser.unpack(())?;
        parser.ensure_empty()?;
        if value.blk_ref.seq_no != seqno {
            return Err(toner::tlb::Error::custom(format!(
                "old masterchain block seqno mismatch: expected {seqno}, got {}",
                value.blk_ref.seq_no
            )));
        }
        return Ok(OldMcBlockLookup::Found(value));
    }

    let Some((is_right, remaining)) = remaining.split_first() else {
        return Err(toner::tlb::Error::custom("hashmap key ended before fork"));
    };
    if cell.references.len() != 2 {
        return Err(toner::tlb::Error::custom(format!(
            "hashmap fork must have 2 references, got {}",
            cell.references.len()
        )));
    }

    lookup_old_mc_block(
        &cell.references[usize::from(*is_right)],
        remaining,
        m - 1,
        seqno,
    )
}

fn lookup_cell_hashmap_edge(
    cell: &Cell,
    key: &BitSlice<u8, Msb0>,
    n: u32,
) -> Result<CellLookup, StringError> {
    if cell.is_exotic {
        return if cell.data.as_raw_slice().first() == Some(&1) {
            Ok(CellLookup::Pruned)
        } else {
            Err(toner::tlb::Error::custom("unexpected exotic hashmap cell"))
        };
    }

    let mut parser = cell.parser();
    let label = parse_hm_label(&mut parser, n)?;
    if !key.starts_with(&label) {
        return Ok(CellLookup::Absent);
    }
    let m = n
        .checked_sub(label.len() as u32)
        .ok_or_else(|| toner::tlb::Error::custom("hashmap label exceeds remaining key"))?;
    let remaining = &key[label.len()..];

    if m == 0 {
        let value = parser.parse_as::<Cell, Ref>(())?;
        parser.ensure_empty()?;
        return Ok(CellLookup::Found(value));
    }

    let Some((is_right, remaining)) = remaining.split_first() else {
        return Err(toner::tlb::Error::custom("hashmap key ended before fork"));
    };
    if cell.references.len() != 2 {
        return Err(toner::tlb::Error::custom(format!(
            "hashmap fork must have 2 references, got {}",
            cell.references.len()
        )));
    }

    lookup_cell_hashmap_edge(&cell.references[usize::from(*is_right)], remaining, m - 1)
}

fn parse_hm_label(parser: &mut CellParser<'_>, m: u32) -> Result<BitVec<u8, Msb0>, StringError> {
    if !parser.unpack::<bool>(())? {
        let n: u32 = parser.unpack_as::<_, toner::tlb::bits::Unary>(())?;
        if n > m {
            return Err(toner::tlb::Error::custom("hashmap label exceeds key"));
        }
        return parser.unpack(n as usize);
    }

    let n_bits = m.checked_ilog2().unwrap_or(0) + 1;
    if !parser.unpack::<bool>(())? {
        let n: u32 = parser.unpack_as::<_, toner::tlb::bits::VarNBits>(n_bits)?;
        if n > m {
            return Err(toner::tlb::Error::custom("hashmap label exceeds key"));
        }
        return parser.unpack(n as usize);
    }

    let value: bool = parser.unpack(())?;
    let n: u32 = parser.unpack_as::<_, toner::tlb::bits::VarNBits>(n_bits)?;
    if n > m {
        return Err(toner::tlb::Error::custom("hashmap label exceeds key"));
    }
    Ok(BitVec::repeat(value, n as usize))
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
    use super::{
        BlockCreateStats, KeyMaxLt, McStateExtra, McStateExtraInfo, OldMcBlockLookup,
        OldMcBlocksInfo,
    };
    use crate::tlb::ext_blk_ref::ExtBlkRef;
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

    #[test]
    fn should_lookup_old_mc_block() {
        let block = old_mc_block(0);
        let root = TestOldMcBlock(block.blk_ref.clone()).to_cell(()).unwrap();
        let old_blocks = OldMcBlocksInfo {
            root: Some(root),
            extra: KeyMaxLt {
                key: false,
                max_end_lt: 0,
            },
        };

        let actual = old_blocks.lookup(0).unwrap();

        assert_eq!(actual, OldMcBlockLookup::Found(block));
    }

    #[test]
    fn should_report_absent_old_mc_block() {
        let root = TestOldMcBlock(old_mc_block(0).blk_ref).to_cell(()).unwrap();
        let old_blocks = OldMcBlocksInfo {
            root: Some(root),
            extra: KeyMaxLt {
                key: false,
                max_end_lt: 0,
            },
        };

        let actual = old_blocks.lookup(1).unwrap();

        assert_eq!(actual, OldMcBlockLookup::Absent);
    }

    #[test]
    fn should_report_pruned_old_mc_block() {
        let mut data = vec![0_u8; 36];
        data[0] = 1;
        let root = Cell {
            is_exotic: true,
            data: BitVec::from_vec(data),
            references: Vec::new(),
        };
        let old_blocks = OldMcBlocksInfo {
            root: Some(root),
            extra: KeyMaxLt {
                key: false,
                max_end_lt: 0,
            },
        };

        let actual = old_blocks.lookup(0).unwrap();

        assert_eq!(actual, OldMcBlockLookup::Pruned);
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

    struct TestOldMcBlock(ExtBlkRef);

    impl CellSerialize for TestOldMcBlock {
        type Args = ();

        fn store(
            &self,
            builder: &mut CellBuilder,
            _args: Self::Args,
        ) -> Result<(), CellBuilderError> {
            builder
                .pack_as::<_, toner::tlb::bits::NBits<2>>(0b11_u8, ())?
                .pack(false, ())?
                .pack_as::<_, toner::tlb::bits::VarNBits>(32_u32, 6)?
                .pack(false, ())?
                .pack(0_u64, ())?
                .pack(false, ())?
                .pack(&self.0, ())?;
            Ok(())
        }
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
