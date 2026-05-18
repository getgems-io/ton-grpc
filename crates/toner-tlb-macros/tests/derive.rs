use toner::tlb::Cell;
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::vec::BitVec;
use toner_tlb_macros::{BitPack, BitUnpack, CellDeserialize, CellSerialize};

fn make_cell(data_bits: &[bool], refs: Vec<Cell>) -> Cell {
    let mut bv = BitVec::<u8, Msb0>::new();
    for &b in data_bits {
        bv.push(b);
    }
    let references = refs.into_iter().map(|c| std::sync::Arc::new(c)).collect();
    Cell {
        is_exotic: false,
        data: bv,
        references,
    }
}

fn make_leaf_cell(data_bits: &[bool]) -> Cell {
    make_cell(data_bits, vec![])
}

fn byte_bits(value: u8) -> Vec<bool> {
    (0..8).rev().map(|i| (value >> i) & 1 == 1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_parse_fields() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct Simple {
            #[tlb(bits)]
            a: u8,
            #[tlb(bits)]
            b: u16,
        }

        let mut bits = vec![false; 8 + 16];
        bits[7] = true;
        bits[8 + 15] = true;
        bits[8 + 14] = true;
        let cell = make_leaf_cell(&bits);

        let result: Simple = cell.parse_fully(()).unwrap();

        assert_eq!(result, Simple { a: 1, b: 3 });
    }

    #[test]
    fn struct_with_tag_validation() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        #[tlb(tag = "0b0111")]
        struct Tagged {
            #[tlb(bits)]
            val: u8,
        }

        let mut bits = vec![false; 4 + 8];
        bits[1] = true;
        bits[2] = true;
        bits[3] = true;
        bits[4 + 7] = true;
        let cell = make_leaf_cell(&bits);

        let result: Tagged = cell.parse_fully(()).unwrap();

        assert_eq!(result, Tagged { val: 1 });
    }

    #[test]
    fn struct_with_tag_validation_fails_on_wrong_tag() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        #[tlb(tag = "0b0111")]
        struct Tagged {
            #[tlb(bits)]
            val: u8,
        }

        let bits = vec![false; 4 + 8];
        let cell = make_leaf_cell(&bits);

        let result: Result<Tagged, _> = cell.parse_fully(());

        assert!(result.is_err());
    }

    #[test]
    fn struct_with_hex_tag() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        #[tlb(tag = "0xff")]
        struct HexTagged {
            #[tlb(bits)]
            val: u8,
        }

        let mut bits = vec![true; 8];
        bits.extend_from_slice(&[false, false, false, false, false, false, true, false]);
        let cell = make_leaf_cell(&bits);

        let result: HexTagged = cell.parse_fully(()).unwrap();

        assert_eq!(result, HexTagged { val: 2 });
    }

    #[test]
    fn enum_flat_tags() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        enum Color {
            #[tlb(tag = "0b00")]
            Red,
            #[tlb(tag = "0b01")]
            Green,
            #[tlb(tag = "0b10")]
            Blue,
        }

        let cell_red = make_leaf_cell(&[false, false]);
        let cell_green = make_leaf_cell(&[false, true]);
        let cell_blue = make_leaf_cell(&[true, false]);

        assert_eq!(cell_red.parse_fully::<Color>(()).unwrap(), Color::Red);
        assert_eq!(cell_green.parse_fully::<Color>(()).unwrap(), Color::Green);
        assert_eq!(cell_blue.parse_fully::<Color>(()).unwrap(), Color::Blue);
    }

    #[test]
    fn enum_flat_tags_invalid() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        enum Color {
            #[tlb(tag = "0b00")]
            Red,
            #[tlb(tag = "0b01")]
            Green,
        }

        let cell = make_leaf_cell(&[true, true]);

        let result: Result<Color, _> = cell.parse_fully(());

        assert!(result.is_err());
    }

    #[test]
    fn enum_with_fields() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        enum Msg {
            #[tlb(tag = "0b00")]
            Empty,
            #[tlb(tag = "0b01")]
            WithVal {
                #[tlb(bits)]
                val: u8,
            },
        }

        let mut bits_with_val = vec![false, true];
        bits_with_val.extend_from_slice(&[false, false, false, false, false, true, false, true]);
        let cell = make_leaf_cell(&bits_with_val);

        let result: Msg = cell.parse_fully(()).unwrap();

        assert_eq!(result, Msg::WithVal { val: 5 });
    }

    #[test]
    fn enum_tree_tags() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        enum TreeTagged {
            #[tlb(tag = "0b00")]
            A,
            #[tlb(tag = "0b010")]
            B,
            #[tlb(tag = "0b011")]
            C,
            #[tlb(tag = "0b10")]
            D,
        }

        let cell_a = make_leaf_cell(&[false, false]);
        let cell_b = make_leaf_cell(&[false, true, false]);
        let cell_c = make_leaf_cell(&[false, true, true]);
        let cell_d = make_leaf_cell(&[true, false]);

        assert_eq!(cell_a.parse_fully::<TreeTagged>(()).unwrap(), TreeTagged::A);
        assert_eq!(cell_b.parse_fully::<TreeTagged>(()).unwrap(), TreeTagged::B);
        assert_eq!(cell_c.parse_fully::<TreeTagged>(()).unwrap(), TreeTagged::C);
        assert_eq!(cell_d.parse_fully::<TreeTagged>(()).unwrap(), TreeTagged::D);
    }

    #[test]
    fn enum_tree_tags_with_fields() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        enum InMsgLike {
            #[tlb(tag = "0b000")]
            ImportExt {
                #[tlb(bits)]
                val: u8,
            },
            #[tlb(tag = "0b00100")]
            DeferredFin {
                #[tlb(bits)]
                x: u8,
            },
            #[tlb(tag = "0b00101")]
            DeferredTr {
                #[tlb(bits)]
                y: u8,
            },
        }

        let mut bits_ext = vec![false, false, false];
        bits_ext.extend_from_slice(&[false, false, false, false, false, false, false, true]);
        let cell_ext = make_leaf_cell(&bits_ext);

        let result: InMsgLike = cell_ext.parse_fully(()).unwrap();

        assert_eq!(result, InMsgLike::ImportExt { val: 1 });

        let mut bits_dfin = vec![false, false, true, false, false];
        bits_dfin.extend_from_slice(&[false, false, false, false, false, false, true, false]);
        let cell_dfin = make_leaf_cell(&bits_dfin);

        let result: InMsgLike = cell_dfin.parse_fully(()).unwrap();

        assert_eq!(result, InMsgLike::DeferredFin { x: 2 });

        let mut bits_dtr = vec![false, false, true, false, true];
        bits_dtr.extend_from_slice(&[false, false, false, false, false, false, true, true]);
        let cell_dtr = make_leaf_cell(&bits_dtr);

        let result: InMsgLike = cell_dtr.parse_fully(()).unwrap();

        assert_eq!(result, InMsgLike::DeferredTr { y: 3 });
    }

    #[test]
    fn field_mode_unpack_as() {
        use toner::tlb::bits::NBits;

        #[derive(Debug, PartialEq, CellDeserialize)]
        struct WithNBits {
            #[tlb(bits, as = "NBits<4>")]
            nibble: u8,
        }

        let bits = vec![true, false, true, false];
        let cell = make_leaf_cell(&bits);

        let result: WithNBits = cell.parse_fully(()).unwrap();

        assert_eq!(result, WithNBits { nibble: 0b1010 });
    }

    #[test]
    fn tuple_struct_single_field() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct Wrapper(#[tlb(bits)] u8);

        let mut bits = vec![false; 8];
        bits[7] = true;
        let cell = make_leaf_cell(&bits);

        let result: Wrapper = cell.parse_fully(()).unwrap();

        assert_eq!(result, Wrapper(1));
    }

    #[test]
    fn tuple_struct_multiple_fields() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct Pair(#[tlb(bits)] u8, #[tlb(bits)] u16);

        let mut bits = vec![false; 8 + 16];
        bits[7] = true;
        bits[8 + 15] = true;
        bits[8 + 14] = true;
        let cell = make_leaf_cell(&bits);

        let result: Pair = cell.parse_fully(()).unwrap();

        assert_eq!(result, Pair(1, 3));
    }

    #[test]
    fn tuple_struct_with_tag() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        #[tlb(tag = "0b11")]
        struct Tagged(#[tlb(bits)] u8);

        let bits = vec![
            true, true, false, false, false, false, false, false, false, true,
        ];
        let cell = make_leaf_cell(&bits);

        let result: Tagged = cell.parse_fully(()).unwrap();

        assert_eq!(result, Tagged(1));
    }

    #[test]
    fn struct_with_short_hex_tag() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        #[tlb(tag = "0xab")]
        struct ShortHex {
            #[tlb(bits)]
            val: u8,
        }

        // 0xab = 10101011 (8 bits) + val=3 = 00000011
        let bits = vec![
            true, false, true, false, true, false, true, true, false, false, false, false, false,
            false, true, true,
        ];
        let cell = make_leaf_cell(&bits);

        let result: ShortHex = cell.parse_fully(()).unwrap();

        assert_eq!(result, ShortHex { val: 3 });
    }

    #[test]
    fn struct_with_12bit_hex_tag() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        #[tlb(tag = "0xabc")]
        struct Hex12 {
            #[tlb(bits)]
            val: u8,
        }

        // 0xabc = 1010 1011 1100 (12 bits) + val=1 = 00000001
        let bits = vec![
            true, false, true, false, true, false, true, true, true, true, false, false, false,
            false, false, false, false, false, false, true,
        ];
        let cell = make_leaf_cell(&bits);

        let result: Hex12 = cell.parse_fully(()).unwrap();

        assert_eq!(result, Hex12 { val: 1 });
    }

    #[test]
    fn enum_tag_exceeds_u8() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/compile-fail/enum_tag_exceeds_u8.rs");
    }

    #[test]
    fn separate_cell_block_loads_fields_from_child_cell() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct WithBlock {
            #[tlb(bits)]
            head: u8,
            #[tlb(separate_cell_start, bits)]
            inner_a: u8,
            #[tlb(separate_cell_end, bits)]
            inner_b: u16,
        }

        let mut head_bits = vec![false; 8];
        head_bits[7] = true;
        let mut inner_bits = vec![false; 8 + 16];
        inner_bits[7] = true;
        inner_bits[8 + 14] = true;
        let inner_cell = make_leaf_cell(&inner_bits);
        let cell = make_cell(&head_bits, vec![inner_cell]);

        let result: WithBlock = cell.parse_fully(()).unwrap();

        assert_eq!(
            result,
            WithBlock {
                head: 1,
                inner_a: 1,
                inner_b: 2,
            }
        );
    }

    #[test]
    fn separate_cell_block_fails_when_child_has_trailing_bits() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct WithBlock {
            #[tlb(separate_cell_start, bits)]
            inner_a: u8,
            #[tlb(separate_cell_end, bits)]
            inner_b: u8,
        }

        let inner_bits = vec![false; 8 + 8 + 1];
        let inner_cell = make_leaf_cell(&inner_bits);
        let cell = make_cell(&[], vec![inner_cell]);

        let result: Result<WithBlock, _> = cell.parse_fully(());

        assert!(result.is_err());
    }

    #[test]
    fn separate_cell_block_fails_when_no_reference_left() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct WithBlock {
            #[tlb(separate_cell_start, bits)]
            inner_a: u8,
            #[tlb(separate_cell_end, bits)]
            inner_b: u8,
        }

        let cell = make_leaf_cell(&[]);

        let result: Result<WithBlock, _> = cell.parse_fully(());

        assert!(result.is_err());
    }

    #[test]
    fn separate_cell_single_field_block() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct WithBlock {
            #[tlb(bits)]
            head: u8,
            #[tlb(separate_cell_start, separate_cell_end, bits)]
            tail: u16,
        }

        let mut head_bits = vec![false; 8];
        head_bits[7] = true;
        let mut inner_bits = vec![false; 16];
        inner_bits[15] = true;
        inner_bits[14] = true;
        let inner_cell = make_leaf_cell(&inner_bits);
        let cell = make_cell(&head_bits, vec![inner_cell]);

        let result: WithBlock = cell.parse_fully(()).unwrap();

        assert_eq!(result, WithBlock { head: 1, tail: 3 });
    }

    #[test]
    fn separate_cell_two_blocks_in_a_row() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct TwoBlocks {
            #[tlb(separate_cell_start, separate_cell_end, bits)]
            first: u8,
            #[tlb(separate_cell_start, separate_cell_end, bits)]
            second: u8,
        }

        let mut first_inner = vec![false; 8];
        first_inner[7] = true;
        let mut second_inner = vec![false; 8];
        second_inner[6] = true;
        let cell = make_cell(
            &[],
            vec![make_leaf_cell(&first_inner), make_leaf_cell(&second_inner)],
        );

        let result: TwoBlocks = cell.parse_fully(()).unwrap();

        assert_eq!(
            result,
            TwoBlocks {
                first: 1,
                second: 2
            }
        );
    }

    #[test]
    fn separate_cell_in_enum_variant() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        enum Msg {
            #[tlb(tag = "0b0")]
            Skipped {
                #[tlb(bits)]
                reason: u8,
            },
            #[tlb(tag = "0b1")]
            Vm {
                #[tlb(bits)]
                head: u8,
                #[tlb(separate_cell_start, bits)]
                tail_a: u8,
                #[tlb(separate_cell_end, bits)]
                tail_b: u16,
            },
        }

        let mut data_bits = vec![true];
        data_bits.extend(std::iter::repeat_n(false, 8));
        data_bits[8] = true;
        let mut inner_bits = vec![false; 8 + 16];
        inner_bits[7] = true;
        inner_bits[8 + 15] = true;
        inner_bits[8 + 14] = true;
        let cell = make_cell(&data_bits, vec![make_leaf_cell(&inner_bits)]);

        let result: Msg = cell.parse_fully(()).unwrap();

        assert_eq!(
            result,
            Msg::Vm {
                head: 1,
                tail_a: 1,
                tail_b: 3,
            }
        );
    }

    #[test]
    fn separate_cell_in_tuple_struct() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct Pair(
            #[tlb(bits)] u8,
            #[tlb(separate_cell_start, bits)] u8,
            #[tlb(separate_cell_end, bits)] u8,
        );

        let mut head_bits = vec![false; 8];
        head_bits[7] = true;
        let mut inner_bits = vec![false; 16];
        inner_bits[7] = true;
        inner_bits[15] = true;
        inner_bits[14] = true;
        let cell = make_cell(&head_bits, vec![make_leaf_cell(&inner_bits)]);

        let result: Pair = cell.parse_fully(()).unwrap();

        assert_eq!(result, Pair(1, 1, 3));
    }

    #[test]
    fn separate_cell_nested_via_composition() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct Inner {
            #[tlb(bits)]
            inner_head: u8,
            #[tlb(separate_cell_start, bits)]
            a: u8,
            #[tlb(separate_cell_end, bits)]
            b: u8,
        }

        #[derive(Debug, PartialEq, CellDeserialize)]
        struct Outer {
            #[tlb(bits)]
            head: u8,
            #[tlb(separate_cell_start, bits)]
            mid: u8,
            #[tlb(separate_cell_end)]
            inner: Inner,
        }

        let head_bits = byte_bits(1);
        let mid_bits = byte_bits(2);
        let inner_head_bits = byte_bits(4);
        let mut ab_bits = byte_bits(16);
        ab_bits.extend(byte_bits(32));

        let ab_cell = make_leaf_cell(&ab_bits);
        let mut block_data = mid_bits.clone();
        block_data.extend(inner_head_bits);
        let block_cell = make_cell(&block_data, vec![ab_cell]);
        let cell = make_cell(&head_bits, vec![block_cell]);

        let result: Outer = cell.parse_fully(()).unwrap();

        assert_eq!(
            result,
            Outer {
                head: 1,
                mid: 2,
                inner: Inner {
                    inner_head: 4,
                    a: 16,
                    b: 32,
                },
            }
        );
    }

    #[test]
    fn separate_cell_orphan_start_fails_to_compile() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/compile-fail/separate_cell_orphan_start.rs");
    }

    #[test]
    fn separate_cell_orphan_end_fails_to_compile() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/compile-fail/separate_cell_orphan_end.rs");
    }

    #[test]
    fn separate_cell_nested_start_fails_to_compile() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/compile-fail/separate_cell_nested_start.rs");
    }

    #[test]
    fn separate_cell_nested_both_fails_to_compile() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/compile-fail/separate_cell_nested_both.rs");
    }

    #[test]
    fn separate_cell_double_end_fails_to_compile() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/compile-fail/separate_cell_double_end.rs");
    }

    #[test]
    fn bit_unpack_struct_named_fields() {
        use toner::tlb::bits::de::unpack_fully;

        #[derive(Debug, PartialEq, BitUnpack)]
        struct ExtBlkRefLike {
            end_lt: u64,
            seq_no: u32,
        }

        let mut bv = BitVec::<u8, Msb0>::new();
        for byte in 1u64.to_be_bytes() {
            for &b in &byte_bits(byte) {
                bv.push(b);
            }
        }
        for byte in 2u32.to_be_bytes() {
            for &b in &byte_bits(byte) {
                bv.push(b);
            }
        }

        let result: ExtBlkRefLike = unpack_fully(bv.as_bitslice(), ()).unwrap();

        assert_eq!(
            result,
            ExtBlkRefLike {
                end_lt: 1,
                seq_no: 2
            }
        );
    }

    #[test]
    fn bit_unpack_enum_flat_tags() {
        use toner::tlb::bits::bitvec::bits;
        use toner::tlb::bits::de::unpack_fully;

        #[derive(Debug, PartialEq, BitUnpack)]
        enum AccountStatusLike {
            #[tlb(tag = "0b00")]
            Uninit,
            #[tlb(tag = "0b01")]
            Frozen,
            #[tlb(tag = "0b10")]
            Active,
            #[tlb(tag = "0b11")]
            Nonexist,
        }

        assert_eq!(
            unpack_fully::<AccountStatusLike>(bits![u8, Msb0; 0, 0], ()).unwrap(),
            AccountStatusLike::Uninit
        );
        assert_eq!(
            unpack_fully::<AccountStatusLike>(bits![u8, Msb0; 0, 1], ()).unwrap(),
            AccountStatusLike::Frozen
        );
        assert_eq!(
            unpack_fully::<AccountStatusLike>(bits![u8, Msb0; 1, 0], ()).unwrap(),
            AccountStatusLike::Active
        );
        assert_eq!(
            unpack_fully::<AccountStatusLike>(bits![u8, Msb0; 1, 1], ()).unwrap(),
            AccountStatusLike::Nonexist
        );
    }

    #[test]
    fn bit_unpack_struct_with_tag_validation() {
        use toner::tlb::bits::bitvec::bits;
        use toner::tlb::bits::de::unpack_fully;

        #[derive(Debug, PartialEq, BitUnpack)]
        #[tlb(tag = "0b0111")]
        struct Tagged {
            val: bool,
        }

        let result: Tagged = unpack_fully(bits![u8, Msb0; 0, 1, 1, 1, 1], ()).unwrap();
        assert_eq!(result, Tagged { val: true });

        let bad: Result<Tagged, _> = unpack_fully(bits![u8, Msb0; 0, 0, 0, 0, 1], ());
        assert!(bad.is_err());
    }

    #[test]
    fn bit_unpack_struct_with_unpack_as() {
        use toner::tlb::bits::NBits;
        use toner::tlb::bits::bitvec::bits;
        use toner::tlb::bits::de::unpack_fully;

        #[derive(Debug, PartialEq, BitUnpack)]
        struct Nibble {
            #[tlb(bits, as = "NBits<4>")]
            val: u8,
        }

        let result: Nibble = unpack_fully(bits![u8, Msb0; 1, 0, 1, 0], ()).unwrap();
        assert_eq!(result, Nibble { val: 0b1010 });
    }

    #[test]
    fn bit_unpack_tuple_struct() {
        use toner::tlb::bits::bitvec::bits;
        use toner::tlb::bits::de::unpack_fully;

        #[derive(Debug, PartialEq, BitUnpack)]
        struct Pair(bool, bool);

        let result: Pair = unpack_fully(bits![u8, Msb0; 1, 0], ()).unwrap();
        assert_eq!(result, Pair(true, false));
    }

    #[test]
    fn bit_unpack_enum_with_fields() {
        use toner::tlb::bits::NBits;
        use toner::tlb::bits::bitvec::bits;
        use toner::tlb::bits::de::unpack_fully;

        #[derive(Debug, PartialEq, BitUnpack)]
        enum Msg {
            #[tlb(tag = "0b0")]
            Ping,
            #[tlb(tag = "0b1")]
            WithVal {
                #[tlb(bits, as = "NBits<4>")]
                val: u8,
            },
        }

        let ping: Msg = unpack_fully(bits![u8, Msb0; 0], ()).unwrap();
        assert_eq!(ping, Msg::Ping);

        let with_val: Msg = unpack_fully(bits![u8, Msb0; 1, 1, 0, 1, 0], ()).unwrap();
        assert_eq!(with_val, Msg::WithVal { val: 0b1010 });
    }

    #[test]
    fn bit_unpack_nested_struct_via_unpack_default() {
        use toner::tlb::bits::bitvec::bits;
        use toner::tlb::bits::de::unpack_fully;

        #[derive(Debug, PartialEq, BitUnpack)]
        struct Inner {
            a: bool,
            b: bool,
        }

        #[derive(Debug, PartialEq, BitUnpack)]
        struct Outer {
            head: bool,
            inner: Inner,
        }

        let result: Outer = unpack_fully(bits![u8, Msb0; 1, 0, 1], ()).unwrap();
        assert_eq!(
            result,
            Outer {
                head: true,
                inner: Inner { a: false, b: true },
            }
        );
    }

    #[test]
    fn bit_unpack_rejects_parse_attr() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/compile-fail/bit_unpack_parse.rs");
    }

    #[test]
    fn bit_unpack_rejects_parse_as_attr() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/compile-fail/bit_unpack_parse_as.rs");
    }

    #[test]
    fn bit_unpack_rejects_separate_cell_attr() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/compile-fail/bit_unpack_separate_cell.rs");
    }

    #[test]
    fn bit_pack_struct_named_fields() {
        use toner::tlb::bits::de::unpack_fully;
        use toner::tlb::bits::ser::pack;

        #[derive(Debug, PartialEq, BitPack, BitUnpack)]
        struct ExtBlkRefLike {
            end_lt: u64,
            seq_no: u32,
        }

        let value = ExtBlkRefLike {
            end_lt: 0x1234_5678_9abc_def0,
            seq_no: 0x0a0b_0c0d,
        };

        let bits = pack(&value, ()).unwrap();
        let decoded: ExtBlkRefLike = unpack_fully(bits.as_bitslice(), ()).unwrap();

        assert_eq!(decoded, value);
    }

    #[test]
    fn bit_pack_struct_with_tag_writes_tag_bits() {
        use toner::tlb::bits::bitvec::bits;
        use toner::tlb::bits::ser::pack;

        #[derive(Debug, PartialEq, BitPack)]
        #[tlb(tag = "0b0111")]
        struct Tagged {
            val: bool,
        }

        let bits_out = pack(&Tagged { val: true }, ()).unwrap();

        assert_eq!(bits_out, bits![u8, Msb0; 0, 1, 1, 1, 1]);
    }

    #[test]
    fn bit_pack_struct_with_unpack_as_roundtrip() {
        use toner::tlb::bits::NBits;
        use toner::tlb::bits::de::unpack_fully;
        use toner::tlb::bits::ser::pack;

        #[derive(Debug, PartialEq, BitPack, BitUnpack)]
        struct Nibble {
            #[tlb(bits, as = "NBits<4>")]
            val: u8,
        }

        let value = Nibble { val: 0b1010 };
        let bits_out = pack(&value, ()).unwrap();
        assert_eq!(bits_out.len(), 4);

        let decoded: Nibble = unpack_fully(bits_out.as_bitslice(), ()).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn bit_pack_tuple_struct_roundtrip() {
        use toner::tlb::bits::de::unpack_fully;
        use toner::tlb::bits::ser::pack;

        #[derive(Debug, PartialEq, BitPack, BitUnpack)]
        struct Pair(bool, bool);

        let value = Pair(true, false);
        let bits_out = pack(&value, ()).unwrap();
        let decoded: Pair = unpack_fully(bits_out.as_bitslice(), ()).unwrap();

        assert_eq!(decoded, value);
    }

    #[test]
    fn bit_pack_enum_flat_tags_roundtrip() {
        use toner::tlb::bits::de::unpack_fully;
        use toner::tlb::bits::ser::pack;

        #[derive(Debug, PartialEq, BitPack, BitUnpack)]
        enum AccountStatusLike {
            #[tlb(tag = "0b00")]
            Uninit,
            #[tlb(tag = "0b01")]
            Frozen,
            #[tlb(tag = "0b10")]
            Active,
            #[tlb(tag = "0b11")]
            Nonexist,
        }

        for value in [
            AccountStatusLike::Uninit,
            AccountStatusLike::Frozen,
            AccountStatusLike::Active,
            AccountStatusLike::Nonexist,
        ] {
            let bits_out = pack(&value, ()).unwrap();
            assert_eq!(bits_out.len(), 2);
            let decoded: AccountStatusLike = unpack_fully(bits_out.as_bitslice(), ()).unwrap();
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn bit_pack_enum_tree_tags_roundtrip() {
        use toner::tlb::bits::de::unpack_fully;
        use toner::tlb::bits::ser::pack;

        #[derive(Debug, PartialEq, BitPack, BitUnpack)]
        enum InMsgLike {
            #[tlb(tag = "0b000")]
            ImportExt { val: u8 },
            #[tlb(tag = "0b00100")]
            DeferredFin { x: u8 },
            #[tlb(tag = "0b00101")]
            DeferredTr { y: u8 },
        }

        for value in [
            InMsgLike::ImportExt { val: 0x42 },
            InMsgLike::DeferredFin { x: 0x10 },
            InMsgLike::DeferredTr { y: 0xff },
        ] {
            let bits_out = pack(&value, ()).unwrap();
            let decoded: InMsgLike = unpack_fully(bits_out.as_bitslice(), ()).unwrap();
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn bit_pack_nested_struct_roundtrip() {
        use toner::tlb::bits::de::unpack_fully;
        use toner::tlb::bits::ser::pack;

        #[derive(Debug, PartialEq, BitPack, BitUnpack)]
        struct Inner {
            a: bool,
            b: bool,
        }

        #[derive(Debug, PartialEq, BitPack, BitUnpack)]
        struct Outer {
            head: bool,
            inner: Inner,
        }

        let value = Outer {
            head: true,
            inner: Inner { a: false, b: true },
        };
        let bits_out = pack(&value, ()).unwrap();
        let decoded: Outer = unpack_fully(bits_out.as_bitslice(), ()).unwrap();

        assert_eq!(decoded, value);
    }

    #[test]
    fn cell_serialize_struct_named_fields_roundtrip() {
        use toner::tlb::ser::CellSerializeExt;

        #[derive(Debug, PartialEq, CellSerialize, CellDeserialize)]
        struct Simple {
            #[tlb(bits)]
            a: u8,
            #[tlb(bits)]
            b: u16,
        }

        let value = Simple { a: 0x42, b: 0xbeef };
        let cell = value.to_cell(()).unwrap();
        let decoded: Simple = cell.parse_fully(()).unwrap();

        assert_eq!(decoded, value);
    }

    #[test]
    fn cell_serialize_struct_with_tag_roundtrip() {
        use toner::tlb::ser::CellSerializeExt;

        #[derive(Debug, PartialEq, CellSerialize, CellDeserialize)]
        #[tlb(tag = "0xff")]
        struct HexTagged {
            #[tlb(bits)]
            val: u8,
        }

        let value = HexTagged { val: 0x2a };
        let cell = value.to_cell(()).unwrap();
        let decoded: HexTagged = cell.parse_fully(()).unwrap();

        assert_eq!(decoded, value);
    }

    #[test]
    fn cell_serialize_tuple_struct_roundtrip() {
        use toner::tlb::ser::CellSerializeExt;

        #[derive(Debug, PartialEq, CellSerialize, CellDeserialize)]
        struct Pair(#[tlb(bits)] u8, #[tlb(bits)] u16);

        let value = Pair(1, 0xc0de);
        let cell = value.to_cell(()).unwrap();
        let decoded: Pair = cell.parse_fully(()).unwrap();

        assert_eq!(decoded, value);
    }

    #[test]
    fn cell_serialize_enum_with_fields_roundtrip() {
        use toner::tlb::ser::CellSerializeExt;

        #[derive(Debug, PartialEq, CellSerialize, CellDeserialize)]
        enum Msg {
            #[tlb(tag = "0b00")]
            Empty,
            #[tlb(tag = "0b01")]
            WithVal {
                #[tlb(bits)]
                val: u8,
            },
        }

        for value in [Msg::Empty, Msg::WithVal { val: 0x77 }] {
            let cell = value.to_cell(()).unwrap();
            let decoded: Msg = cell.parse_fully(()).unwrap();
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn cell_serialize_enum_tree_tags_roundtrip() {
        use toner::tlb::ser::CellSerializeExt;

        #[derive(Debug, PartialEq, CellSerialize, CellDeserialize)]
        enum InMsgLike {
            #[tlb(tag = "0b000")]
            ImportExt {
                #[tlb(bits)]
                val: u8,
            },
            #[tlb(tag = "0b00100")]
            DeferredFin {
                #[tlb(bits)]
                x: u8,
            },
            #[tlb(tag = "0b00101")]
            DeferredTr {
                #[tlb(bits)]
                y: u8,
            },
        }

        for value in [
            InMsgLike::ImportExt { val: 0x11 },
            InMsgLike::DeferredFin { x: 0x22 },
            InMsgLike::DeferredTr { y: 0x33 },
        ] {
            let cell = value.to_cell(()).unwrap();
            let decoded: InMsgLike = cell.parse_fully(()).unwrap();
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn cell_serialize_separate_cell_block_roundtrip() {
        use toner::tlb::ser::CellSerializeExt;

        #[derive(Debug, PartialEq, CellSerialize, CellDeserialize)]
        struct WithBlock {
            #[tlb(bits)]
            head: u8,
            #[tlb(separate_cell_start, bits)]
            inner_a: u8,
            #[tlb(separate_cell_end, bits)]
            inner_b: u16,
        }

        let value = WithBlock {
            head: 0x11,
            inner_a: 0x22,
            inner_b: 0x3344,
        };
        let cell = value.to_cell(()).unwrap();

        assert_eq!(cell.references.len(), 1);

        let decoded: WithBlock = cell.parse_fully(()).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn cell_serialize_separate_cell_two_blocks_roundtrip() {
        use toner::tlb::ser::CellSerializeExt;

        #[derive(Debug, PartialEq, CellSerialize, CellDeserialize)]
        struct TwoBlocks {
            #[tlb(separate_cell_start, separate_cell_end, bits)]
            first: u8,
            #[tlb(separate_cell_start, separate_cell_end, bits)]
            second: u8,
        }

        let value = TwoBlocks {
            first: 0x55,
            second: 0xaa,
        };
        let cell = value.to_cell(()).unwrap();

        assert_eq!(cell.references.len(), 2);

        let decoded: TwoBlocks = cell.parse_fully(()).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn cell_serialize_unpack_as_field_roundtrip() {
        use toner::tlb::bits::NBits;
        use toner::tlb::ser::CellSerializeExt;

        #[derive(Debug, PartialEq, CellSerialize, CellDeserialize)]
        struct WithNibble {
            #[tlb(bits, as = "NBits<4>")]
            nibble: u8,
            #[tlb(bits)]
            full: u8,
        }

        let value = WithNibble {
            nibble: 0b1010,
            full: 0xc4,
        };
        let cell = value.to_cell(()).unwrap();
        let decoded: WithNibble = cell.parse_fully(()).unwrap();

        assert_eq!(decoded, value);
    }

    #[test]
    fn cell_serialize_iter_field_roundtrip() {
        use toner::tlb::ser::CellSerializeExt;

        #[derive(Debug, PartialEq, Clone, CellSerialize, CellDeserialize)]
        struct Item {
            #[tlb(bits)]
            val: u8,
        }

        #[derive(Debug, PartialEq, CellSerialize, CellDeserialize)]
        struct WithIter {
            #[tlb(bits)]
            header: u16,
            #[tlb(iter)]
            items: Vec<Item>,
        }

        let value = WithIter {
            header: 0xbeef,
            items: vec![Item { val: 1 }, Item { val: 2 }, Item { val: 3 }],
        };
        let cell = value.to_cell(()).unwrap();
        let decoded: WithIter = cell.parse_fully(()).unwrap();

        assert_eq!(decoded, value);
    }

    #[test]
    fn cell_serialize_iter_empty_roundtrip() {
        use toner::tlb::ser::CellSerializeExt;

        #[derive(Debug, PartialEq, Clone, CellSerialize, CellDeserialize)]
        struct Item {
            #[tlb(bits)]
            val: u8,
        }

        #[derive(Debug, PartialEq, CellSerialize, CellDeserialize)]
        struct WithIter {
            #[tlb(bits)]
            header: u8,
            #[tlb(iter)]
            items: Vec<Item>,
        }

        let value = WithIter {
            header: 0x42,
            items: vec![],
        };
        let cell = value.to_cell(()).unwrap();
        let decoded: WithIter = cell.parse_fully(()).unwrap();

        assert_eq!(decoded, value);
    }
}
