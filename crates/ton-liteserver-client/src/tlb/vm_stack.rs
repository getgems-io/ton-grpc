use crate::tlb::vm_cont::VmCont;
use num_bigint::BigUint;
use toner::tlb::Cell;
use toner::tlb::bits::NBits;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::ser::BitWriterExt;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::ser::{CellBuilder, CellBuilderError, CellSerialize};
use toner::tlb::{Context, Error, Ref};
use toner_tlb_macros::{
    CellDeserialize as CellDeserializeDerive, CellSerialize as CellSerializeDerive,
};

/// ```tlb
/// _ cell:^Cell st_bits:(## 10) end_bits:(## 10) { st_bits <= end_bits }
///   st_ref:(#<= 4) end_ref:(#<= 4) { st_ref <= end_ref } = VmCellSlice;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, CellDeserializeDerive, CellSerializeDerive)]
pub struct VmCellSlice {
    #[tlb(cell, as = "Ref")]
    pub cell: Cell,
    #[tlb(bits, as = "NBits<10>")]
    pub st_bits: u16,
    #[tlb(bits, as = "NBits<10>")]
    pub end_bits: u16,
    #[tlb(bits, as = "NBits<3>")]
    pub st_ref: u8,
    #[tlb(bits, as = "NBits<3>")]
    pub end_ref: u8,
}

/// ```tlb
/// vm_stk_null#00 = VmStackValue;
/// vm_stk_tinyint#01 value:int64 = VmStackValue;
/// vm_stk_int#0201_ value:int257 = VmStackValue;
/// vm_stk_nan#02ff = VmStackValue;
/// vm_stk_cell#03 cell:^Cell = VmStackValue;
/// vm_stk_slice#04 _:VmCellSlice = VmStackValue;
/// vm_stk_builder#05 cell:^Cell = VmStackValue;
/// vm_stk_cont#06 cont:VmCont = VmStackValue;
/// vm_stk_tuple#07 len:(## 16) data:(VmTuple len) = VmStackValue;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, CellDeserializeDerive, CellSerializeDerive)]
pub enum VmStackValue {
    #[tlb(tag = "#00")]
    Null,
    #[tlb(tag = "#01")]
    TinyInt {
        #[tlb(bits)]
        value: i64,
    },
    #[tlb(tag = "#0201_")]
    Int {
        // TODO[akostylev0]: switch to signed `BigInt` once `NBits<BITS>: BitPackAs<BigInt>` in
        // toner is fixed (two's-complement layout is currently broken for negatives).
        #[tlb(bits, as = "NBits<257>")]
        value: BigUint,
    },
    #[tlb(tag = "#02ff")]
    Nan,
    #[tlb(tag = "#03")]
    Cell {
        #[tlb(cell, as = "Ref")]
        cell: Cell,
    },
    #[tlb(tag = "#04")]
    Slice { slice: VmCellSlice },
    #[tlb(tag = "#05")]
    Builder {
        #[tlb(cell, as = "Ref")]
        cell: Cell,
    },
    #[tlb(tag = "#06")]
    Cont { cont: Box<VmCont> },
    #[tlb(tag = "#07")]
    Tuple { tuple: VmStkTuple },
}

/// ```tlb
/// vm_tuple_nil$_  = VmTuple 0;
/// vm_tuple_tcons$_ {n:#} head:(VmTupleRef n) tail:^VmStackValue = VmTuple (n + 1);
/// vm_tupref_nil$_ = VmTupleRef 0;
/// vm_tupref_single$_ entry:^VmStackValue = VmTupleRef 1;
/// vm_tupref_any$_ {n:#} ref:^(VmTuple (n + 2)) = VmTupleRef (n + 2);
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VmTuple(pub Vec<VmStackValue>);

impl<'de> CellDeserialize<'de> for VmTuple {
    type Args = (usize,);

    fn parse(
        parser: &mut CellParser<'de>,
        (len,): Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        if len == 0 {
            return Ok(Self(Vec::new()));
        }
        let mut items = if len == 1 {
            Vec::new()
        } else if len == 2 {
            let head: VmStackValue = parser
                .parse_as::<_, Ref>(())
                .context("tuple head (single)")?;
            vec![head]
        } else {
            parser
                .parse_as::<VmTuple, Ref>((len - 1,))
                .context("tuple head")?
                .0
        };
        let tail: VmStackValue = parser.parse_as::<_, Ref>(()).context("tuple tail")?;
        items.push(tail);
        Ok(Self(items))
    }
}

impl CellSerialize for VmTuple {
    type Args = (usize,);

    fn store(&self, builder: &mut CellBuilder, (len,): Self::Args) -> Result<(), CellBuilderError> {
        if len != self.0.len() {
            return Err(Error::custom(format!(
                "VmTuple len arg {} mismatches items.len() {}",
                len,
                self.0.len()
            )));
        }
        if len == 0 {
            return Ok(());
        }
        let (tail, head_items) = self.0.split_last().expect("len >= 1");
        match len {
            1 => {}
            2 => builder
                .store_as::<_, Ref>(&head_items[0], ())
                .context("tuple head (single)")
                .map(|_| ())?,
            _ => builder
                .store_as::<_, Ref>(&VmTuple(head_items.to_vec()), (len - 1,))
                .context("tuple head")
                .map(|_| ())?,
        }
        builder.store_as::<_, Ref>(tail, ()).context("tuple tail")?;
        Ok(())
    }
}

/// Inline `len:(## 16)` + `data:(VmTuple len)`. Used as the body of `vm_stk_tuple#07`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VmStkTuple(pub Vec<VmStackValue>);

impl<'de> CellDeserialize<'de> for VmStkTuple {
    type Args = ();

    fn parse(parser: &mut CellParser<'de>, _: Self::Args) -> Result<Self, CellParserError<'de>> {
        let len: u16 = parser.unpack(()).context("tuple len")?;
        let VmTuple(items) = parser.parse((len as usize,)).context("tuple body")?;
        Ok(Self(items))
    }
}

impl CellSerialize for VmStkTuple {
    type Args = ();

    fn store(&self, builder: &mut CellBuilder, _: Self::Args) -> Result<(), CellBuilderError> {
        let len = u16::try_from(self.0.len())
            .map_err(|_| Error::custom(format!("tuple length exceeds u16 ({})", self.0.len())))?;
        builder.pack(len, ()).context("tuple len")?;
        VmTuple(self.0.clone())
            .store(builder, (len as usize,))
            .context("tuple body")?;
        Ok(())
    }
}

/// ```tlb
/// vm_stk_cons#_  {n:#} rest:^(VmStackList n) tos:VmStackValue = VmStackList (n + 1);
/// vm_stk_nil#_   = VmStackList 0;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VmStackList(pub Vec<VmStackValue>);

impl<'de> CellDeserialize<'de> for VmStackList {
    type Args = (usize,);

    fn parse(
        parser: &mut CellParser<'de>,
        (depth,): Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        if depth == 0 {
            return Ok(Self(Vec::new()));
        }
        let VmStackList(mut items) = parser.parse_as::<_, Ref>((depth - 1,)).context("rest")?;
        let tos: VmStackValue = parser.parse(()).context("tos")?;
        items.push(tos);
        Ok(Self(items))
    }
}

impl CellSerialize for VmStackList {
    type Args = (usize,);

    fn store(
        &self,
        builder: &mut CellBuilder,
        (depth,): Self::Args,
    ) -> Result<(), CellBuilderError> {
        if depth != self.0.len() {
            return Err(Error::custom(format!(
                "VmStackList depth arg {} mismatches items.len() {}",
                depth,
                self.0.len()
            )));
        }
        if depth == 0 {
            return Ok(());
        }
        let (tos, rest) = self.0.split_last().expect("depth > 0");
        builder
            .store_as::<_, Ref>(&VmStackList(rest.to_vec()), (depth - 1,))
            .context("rest")?;
        builder.store(tos, ()).context("tos")?;
        Ok(())
    }
}

/// ```tlb
/// vm_stack#_     depth:(## 24) stack:(VmStackList depth) = VmStack;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VmStack(pub Vec<VmStackValue>);

impl<'de> CellDeserialize<'de> for VmStack {
    type Args = ();

    fn parse(parser: &mut CellParser<'de>, _: Self::Args) -> Result<Self, CellParserError<'de>> {
        let depth: u32 = parser.unpack_as::<_, NBits<24>>(()).context("depth")?;
        let VmStackList(items) = parser.parse((depth as usize,)).context("stack")?;
        Ok(Self(items))
    }
}

impl CellSerialize for VmStack {
    type Args = ();

    fn store(&self, builder: &mut CellBuilder, _: Self::Args) -> Result<(), CellBuilderError> {
        let depth = u32::try_from(self.0.len())
            .map_err(|_| Error::custom(format!("stack depth exceeds 2^32-1 ({})", self.0.len())))?;
        if depth >= (1 << 24) {
            return Err(Error::custom(format!(
                "stack depth exceeds 2^24-1 ({depth})"
            )));
        }
        builder
            .pack_as::<_, NBits<24>>(depth, ())
            .context("depth")?;
        VmStackList(self.0.clone())
            .store(builder, (depth as usize,))
            .context("stack")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use std::str::FromStr;
    use toner::tlb::Cell;
    use toner::tlb::bits::ser::BitWriterExt;
    use toner::tlb::ser::CellSerializeExt;

    fn round_trip(value: &VmStackValue) -> VmStackValue {
        let cell = value.to_cell(()).unwrap();

        cell.parse_fully::<VmStackValue>(()).unwrap()
    }

    fn round_trip_stack(stack: &VmStack) -> VmStack {
        let cell = stack.to_cell(()).unwrap();

        cell.parse_fully::<VmStack>(()).unwrap()
    }

    fn make_data_cell(bytes: &[u8]) -> Cell {
        let mut b = Cell::builder();
        for byte in bytes {
            b.pack(*byte, ()).unwrap();
        }
        b.into_cell()
    }

    fn tinyint(v: i64) -> VmStackValue {
        VmStackValue::TinyInt { value: v }
    }

    fn int(v: BigUint) -> VmStackValue {
        VmStackValue::Int { value: v }
    }

    fn tuple(items: Vec<VmStackValue>) -> VmStackValue {
        VmStackValue::Tuple {
            tuple: VmStkTuple(items),
        }
    }

    #[test]
    fn null_round_trips() {
        let value = VmStackValue::Null;

        let parsed = round_trip(&value);

        assert_eq!(parsed, VmStackValue::Null);
    }

    #[test]
    fn null_wire_format_is_single_zero_byte() {
        let value = VmStackValue::Null;

        let cell = value.to_cell(()).unwrap();

        assert_eq!(cell.data.len(), 8);
        assert_eq!(cell.data.as_raw_slice(), &[0x00]);
        assert!(cell.references.is_empty());
    }

    #[test]
    fn tinyint_round_trips_positive() {
        let value = tinyint(42);

        let parsed = round_trip(&value);

        assert_eq!(parsed, tinyint(42));
    }

    #[test]
    fn tinyint_round_trips_negative() {
        let value = tinyint(-1);

        let parsed = round_trip(&value);

        assert_eq!(parsed, tinyint(-1));
    }

    #[test]
    fn tinyint_round_trips_extremes() {
        let min = round_trip(&tinyint(i64::MIN));
        let max = round_trip(&tinyint(i64::MAX));

        assert_eq!(min, tinyint(i64::MIN));
        assert_eq!(max, tinyint(i64::MAX));
    }

    #[test]
    fn tinyint_wire_format_matches_tonutils_layout() {
        let value = tinyint(42);

        let cell = value.to_cell(()).unwrap();

        assert_eq!(cell.data.len(), 8 + 64);
        let bytes = cell.data.as_raw_slice();
        assert_eq!(bytes[0], 0x01);
        assert_eq!(&bytes[1..9], &[0, 0, 0, 0, 0, 0, 0, 42]);
        assert!(cell.references.is_empty());
    }

    #[test]
    fn nan_round_trips() {
        let value = VmStackValue::Nan;

        let parsed = round_trip(&value);

        assert_eq!(parsed, VmStackValue::Nan);
    }

    #[test]
    fn nan_wire_format_is_02_ff() {
        let value = VmStackValue::Nan;

        let cell = value.to_cell(()).unwrap();

        assert_eq!(cell.data.len(), 16);
        assert_eq!(cell.data.as_raw_slice(), &[0x02, 0xff]);
        assert!(cell.references.is_empty());
    }

    #[test]
    fn int257_round_trips_positive_above_i64() {
        let huge = BigUint::from_str("123456789012345678901234567890").unwrap();
        let value = int(huge.clone());

        let parsed = round_trip(&value);

        assert_eq!(parsed, int(huge));
    }

    #[test]
    fn int257_round_trips_zero() {
        let value = int(BigUint::from(0u32));

        let parsed = round_trip(&value);

        assert_eq!(parsed, int(BigUint::from(0u32)));
    }

    #[test]
    fn int257_round_trips_max_magnitude() {
        let mag: BigUint = (BigUint::from(1u32) << 256) - BigUint::from(1u32);
        let value = int(mag.clone());

        let parsed = round_trip(&value);

        assert_eq!(parsed, int(mag));
    }

    #[test]
    fn int257_rejects_overflow() {
        let too_big: BigUint = BigUint::from(1u32) << 257;
        let value = int(too_big);

        let err = value.to_cell(()).unwrap_err();

        assert!(format!("{err}").contains("packed into 257 bits"));
    }

    #[test]
    fn int257_wire_format_zero_uses_02_00_prefix() {
        let value = int(BigUint::from(0u32));

        let cell = value.to_cell(()).unwrap();

        assert_eq!(cell.data.len(), 15 + 257);
        let bytes = cell.data.as_raw_slice();
        assert_eq!(bytes[0], 0x02);
        assert_eq!(bytes[1] & 0xfe, 0x00);
        assert!(bytes[2..].iter().all(|b| *b == 0));
    }

    #[test]
    fn cell_round_trips() {
        let inner = make_data_cell(&[0xde, 0xad, 0xbe, 0xef]);
        let value = VmStackValue::Cell {
            cell: inner.clone(),
        };

        let parsed = round_trip(&value);

        assert_eq!(parsed, VmStackValue::Cell { cell: inner });
    }

    #[test]
    fn builder_round_trips() {
        let inner = make_data_cell(&[0xca, 0xfe]);
        let value = VmStackValue::Builder {
            cell: inner.clone(),
        };

        let parsed = round_trip(&value);

        assert_eq!(parsed, VmStackValue::Builder { cell: inner });
    }

    #[test]
    fn slice_round_trips_whole_cell() {
        let inner = make_data_cell(&[0x01, 0x02, 0x03]);
        let slice = VmCellSlice {
            cell: inner.clone(),
            st_bits: 0,
            end_bits: inner.data.len() as u16,
            st_ref: 0,
            end_ref: inner.references.len() as u8,
        };
        let value = VmStackValue::Slice { slice };

        let parsed = round_trip(&value);

        match parsed {
            VmStackValue::Slice { slice: s } => {
                assert_eq!(s.cell, inner);
                assert_eq!(s.st_bits, 0);
                assert_eq!(s.end_bits, 24);
                assert_eq!(s.st_ref, 0);
                assert_eq!(s.end_ref, 0);
            }
            other => panic!("expected slice, got {other:?}"),
        }
    }

    #[test]
    fn slice_round_trips_with_offsets() {
        let inner = make_data_cell(&[0xff; 3]);
        let slice = VmCellSlice {
            cell: inner,
            st_bits: 4,
            end_bits: 20,
            st_ref: 0,
            end_ref: 0,
        };
        let value = VmStackValue::Slice {
            slice: slice.clone(),
        };

        let parsed = round_trip(&value);

        assert_eq!(parsed, VmStackValue::Slice { slice });
    }

    #[test]
    fn empty_tuple_round_trips() {
        let value = tuple(vec![]);

        let parsed = round_trip(&value);

        assert_eq!(parsed, tuple(vec![]));
    }

    #[test]
    fn singleton_tuple_round_trips() {
        let value = tuple(vec![tinyint(7)]);

        let parsed = round_trip(&value);

        assert_eq!(parsed, tuple(vec![tinyint(7)]));
    }

    #[test]
    fn pair_tuple_round_trips() {
        let value = tuple(vec![tinyint(1), tinyint(2)]);

        let parsed = round_trip(&value);

        assert_eq!(parsed, tuple(vec![tinyint(1), tinyint(2)]));
    }

    #[test]
    fn long_tuple_round_trips() {
        let items: Vec<VmStackValue> = (0..7).map(tinyint).collect();
        let value = tuple(items.clone());

        let parsed = round_trip(&value);

        assert_eq!(parsed, tuple(items));
    }

    #[test]
    fn nested_tuples_round_trip() {
        let value = tuple(vec![
            tuple(vec![tinyint(10), VmStackValue::Null]),
            tuple(vec![]),
            tinyint(3),
        ]);

        let parsed = round_trip(&value);

        assert_eq!(parsed, value);
    }

    #[test]
    fn empty_stack_round_trips() {
        let stack = VmStack::default();

        let parsed = round_trip_stack(&stack);

        assert_eq!(parsed, VmStack::default());
    }

    #[test]
    fn empty_stack_serializes_to_24_zero_bits_no_refs() {
        let stack = VmStack::default();

        let cell = stack.to_cell(()).unwrap();

        assert_eq!(cell.data.len(), 24);
        assert!(cell.data.iter().all(|b| !*b));
        assert!(cell.references.is_empty());
    }

    #[test]
    fn single_element_stack_round_trips() {
        let stack = VmStack(vec![tinyint(7)]);

        let parsed = round_trip_stack(&stack);

        assert_eq!(parsed, stack);
    }

    #[test]
    fn deep_stack_round_trips_preserving_order() {
        let stack = VmStack(vec![
            tinyint(1),
            VmStackValue::Null,
            int(BigUint::from_str("10000000000000000000").unwrap()),
            VmStackValue::Cell {
                cell: make_data_cell(&[0xab]),
            },
            tuple(vec![tinyint(99)]),
        ]);

        let parsed = round_trip_stack(&stack);

        assert_eq!(parsed, stack);
    }

    #[test]
    fn single_tinyint_stack_matches_tonutils_layout() {
        let stack = VmStack(vec![tinyint(42)]);

        let cell = stack.to_cell(()).unwrap();

        assert_eq!(cell.data.len(), 24 + 8 + 64);
        let bytes = cell.data.as_raw_slice();
        assert_eq!(&bytes[0..3], &[0x00, 0x00, 0x01]);
        assert_eq!(bytes[3], 0x01);
        assert_eq!(
            &bytes[4..12],
            &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2a]
        );
        assert_eq!(cell.references.len(), 1);
        let nil = cell.references[0].as_ref();
        assert_eq!(nil.data.len(), 0);
        assert!(nil.references.is_empty());
    }
}
