use toner::tlb::Cell;
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::vec::BitVec;
use toner_tlb_macros::CellDeserialize;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_parse_fields() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct Simple {
            #[tlb(unpack)]
            a: u8,
            #[tlb(unpack)]
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
            #[tlb(unpack)]
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
            #[tlb(unpack)]
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
        #[tlb(tag = "0xff", ensure_empty)]
        struct HexTagged {
            #[tlb(unpack)]
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
                #[tlb(unpack)]
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
                #[tlb(unpack)]
                val: u8,
            },
            #[tlb(tag = "0b00100")]
            DeferredFin {
                #[tlb(unpack)]
                x: u8,
            },
            #[tlb(tag = "0b00101")]
            DeferredTr {
                #[tlb(unpack)]
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
            #[tlb(unpack_as = "NBits<4>")]
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
        struct Wrapper(#[tlb(unpack)] u8);

        let mut bits = vec![false; 8];
        bits[7] = true;
        let cell = make_leaf_cell(&bits);

        let result: Wrapper = cell.parse_fully(()).unwrap();

        assert_eq!(result, Wrapper(1));
    }

    #[test]
    fn tuple_struct_multiple_fields() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        struct Pair(#[tlb(unpack)] u8, #[tlb(unpack)] u16);

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
        struct Tagged(#[tlb(unpack)] u8);

        let bits = vec![
            true, true, false, false, false, false, false, false, false, true,
        ];
        let cell = make_leaf_cell(&bits);

        let result: Tagged = cell.parse_fully(()).unwrap();

        assert_eq!(result, Tagged(1));
    }

    #[test]
    fn tuple_struct_ensure_empty_ok() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        #[tlb(ensure_empty)]
        struct Wrapper(#[tlb(unpack)] u8);

        let mut bits = vec![false; 8];
        bits[7] = true;
        let cell = make_leaf_cell(&bits);

        let result: Wrapper = cell.parse_fully(()).unwrap();

        assert_eq!(result, Wrapper(1));
    }

    #[test]
    fn tuple_struct_ensure_empty_fails_on_trailing_bits() {
        #[derive(Debug, PartialEq, CellDeserialize)]
        #[tlb(ensure_empty)]
        struct Wrapper(#[tlb(unpack)] u8);

        let bits = vec![false; 8 + 1];
        let cell = make_leaf_cell(&bits);

        let result: Result<Wrapper, _> = cell.parse_fully(());

        assert!(result.is_err());
    }
}
