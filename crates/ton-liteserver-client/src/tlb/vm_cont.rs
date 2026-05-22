use crate::tlb::vm_stack::{VmCellSlice, VmStack, VmStackValue};
use toner::tlb::Same;
use toner::tlb::bits::NBits;
use toner::tlb::hashmap::HashmapE;
use toner_tlb_macros::{
    CellDeserialize as CellDeserializeDerive, CellSerialize as CellSerializeDerive,
};

/// ```tlb
/// _ cregs:(HashmapE 4 VmStackValue) = VmSaveList;
/// ```
pub type VmSaveList = HashmapE<VmStackValue>;

/// ```tlb
/// vm_ctl_data$_ nargs:(Maybe uint13) stack:(Maybe VmStack) save:VmSaveList
///   cp:(Maybe int16) = VmControlData;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, CellDeserializeDerive, CellSerializeDerive)]
pub struct VmControlData {
    #[tlb(bits, as = "Option<NBits<13>>")]
    pub nargs: Option<u16>,
    #[tlb(cell)]
    pub stack: Option<VmStack>,
    #[tlb(cell, as = "HashmapE<Same, Same>", args = "(4u32, (), ())")]
    pub save: VmSaveList,
    #[tlb(bits, as = "Option<NBits<16>>")]
    pub cp: Option<i16>,
}

/// ```tlb
/// vmc_std$00 cdata:VmControlData code:VmCellSlice = VmCont;
/// vmc_envelope$01 cdata:VmControlData next:^VmCont = VmCont;
/// vmc_quit$1000 exit_code:int32 = VmCont;
/// vmc_quit_exc$1001 = VmCont;
/// vmc_repeat$10100 count:uint63 body:^VmCont after:^VmCont = VmCont;
/// vmc_until$110000 body:^VmCont after:^VmCont = VmCont;
/// vmc_again$110001 body:^VmCont = VmCont;
/// vmc_while_cond$110010 cond:^VmCont body:^VmCont after:^VmCont = VmCont;
/// vmc_while_body$110011 cond:^VmCont body:^VmCont after:^VmCont = VmCont;
/// vmc_pushint$1111 value:int32 next:^VmCont = VmCont;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, CellDeserializeDerive, CellSerializeDerive)]
pub enum VmCont {
    #[tlb(tag = "$00")]
    Std {
        cdata: VmControlData,
        code: VmCellSlice,
    },
    #[tlb(tag = "$01")]
    Envelope {
        cdata: VmControlData,
        #[tlb(cell, as = "toner::tlb::Ref")]
        next: Box<VmCont>,
    },
    #[tlb(tag = "$1000")]
    Quit {
        #[tlb(bits)]
        exit_code: i32,
    },
    #[tlb(tag = "$1001")]
    QuitExc,
    #[tlb(tag = "$10100")]
    Repeat {
        #[tlb(bits, as = "NBits<63>")]
        count: u64,
        #[tlb(cell, as = "toner::tlb::Ref")]
        body: Box<VmCont>,
        #[tlb(cell, as = "toner::tlb::Ref")]
        after: Box<VmCont>,
    },
    #[tlb(tag = "$110000")]
    Until {
        #[tlb(cell, as = "toner::tlb::Ref")]
        body: Box<VmCont>,
        #[tlb(cell, as = "toner::tlb::Ref")]
        after: Box<VmCont>,
    },
    #[tlb(tag = "$110001")]
    Again {
        #[tlb(cell, as = "toner::tlb::Ref")]
        body: Box<VmCont>,
    },
    #[tlb(tag = "$110010")]
    WhileCond {
        #[tlb(cell, as = "toner::tlb::Ref")]
        cond: Box<VmCont>,
        #[tlb(cell, as = "toner::tlb::Ref")]
        body: Box<VmCont>,
        #[tlb(cell, as = "toner::tlb::Ref")]
        after: Box<VmCont>,
    },
    #[tlb(tag = "$110011")]
    WhileBody {
        #[tlb(cell, as = "toner::tlb::Ref")]
        cond: Box<VmCont>,
        #[tlb(cell, as = "toner::tlb::Ref")]
        body: Box<VmCont>,
        #[tlb(cell, as = "toner::tlb::Ref")]
        after: Box<VmCont>,
    },
    #[tlb(tag = "$1111")]
    PushInt {
        #[tlb(bits)]
        value: i32,
        #[tlb(cell, as = "toner::tlb::Ref")]
        next: Box<VmCont>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tlb::vm_stack::VmStackValue;
    use toner::tlb::Cell;
    use toner::tlb::bits::ser::BitWriterExt;
    use toner::tlb::ser::CellSerializeExt;

    fn round_trip(cont: &VmCont) -> VmCont {
        let cell = cont.to_cell(()).unwrap();

        cell.parse_fully::<VmCont>(()).unwrap()
    }

    fn make_data_cell(bytes: &[u8]) -> Cell {
        let mut b = Cell::builder();
        for byte in bytes {
            b.pack(*byte, ()).unwrap();
        }
        b.into_cell()
    }

    fn empty_control_data() -> VmControlData {
        VmControlData {
            nargs: None,
            stack: None,
            save: HashmapE::Empty,
            cp: None,
        }
    }

    fn quit(code: i32) -> VmCont {
        VmCont::Quit { exit_code: code }
    }

    #[test]
    fn quit_round_trips() {
        let cont = quit(0);

        let parsed = round_trip(&cont);

        assert_eq!(parsed, quit(0));
    }

    #[test]
    fn quit_wire_format_matches_tlb_layout() {
        let cont = quit(42);

        let cell = cont.to_cell(()).unwrap();

        assert_eq!(cell.data.len(), 4 + 32);
        assert!(cell.references.is_empty());
        let bits = cell.data.as_raw_slice();
        assert_eq!(bits[0] >> 4, 0b1000);
    }

    #[test]
    fn quit_round_trips_negative() {
        let cont = quit(-1);

        let parsed = round_trip(&cont);

        assert_eq!(parsed, quit(-1));
    }

    #[test]
    fn quit_exc_round_trips() {
        let cont = VmCont::QuitExc;

        let parsed = round_trip(&cont);

        assert_eq!(parsed, VmCont::QuitExc);
    }

    #[test]
    fn quit_exc_wire_format_is_4_bits_1001() {
        let cont = VmCont::QuitExc;

        let cell = cont.to_cell(()).unwrap();

        assert_eq!(cell.data.len(), 4);
        assert!(cell.references.is_empty());
        let bits = cell.data.as_raw_slice();
        assert_eq!(bits[0] >> 4, 0b1001);
    }

    #[test]
    fn pushint_round_trips() {
        let cont = VmCont::PushInt {
            value: 7,
            next: Box::new(quit(0)),
        };

        let parsed = round_trip(&cont);

        assert_eq!(parsed, cont);
    }

    #[test]
    fn pushint_negative_round_trips() {
        let cont = VmCont::PushInt {
            value: -123,
            next: Box::new(quit(0)),
        };

        let parsed = round_trip(&cont);

        assert_eq!(parsed, cont);
    }

    #[test]
    fn again_round_trips() {
        let cont = VmCont::Again {
            body: Box::new(quit(0)),
        };

        let parsed = round_trip(&cont);

        assert_eq!(parsed, cont);
    }

    #[test]
    fn until_round_trips() {
        let cont = VmCont::Until {
            body: Box::new(quit(1)),
            after: Box::new(VmCont::QuitExc),
        };

        let parsed = round_trip(&cont);

        assert_eq!(parsed, cont);
    }

    #[test]
    fn repeat_round_trips() {
        let cont = VmCont::Repeat {
            count: 1_000_000_000_000,
            body: Box::new(quit(0)),
            after: Box::new(quit(1)),
        };

        let parsed = round_trip(&cont);

        assert_eq!(parsed, cont);
    }

    #[test]
    fn repeat_max_count_round_trips() {
        let max = (1u64 << 63) - 1;
        let cont = VmCont::Repeat {
            count: max,
            body: Box::new(quit(0)),
            after: Box::new(quit(0)),
        };

        let parsed = round_trip(&cont);

        assert_eq!(parsed, cont);
    }

    #[test]
    fn while_cond_round_trips() {
        let cont = VmCont::WhileCond {
            cond: Box::new(quit(1)),
            body: Box::new(quit(2)),
            after: Box::new(quit(3)),
        };

        let parsed = round_trip(&cont);

        assert_eq!(parsed, cont);
    }

    #[test]
    fn while_body_round_trips() {
        let cont = VmCont::WhileBody {
            cond: Box::new(quit(1)),
            body: Box::new(quit(2)),
            after: Box::new(quit(3)),
        };

        let parsed = round_trip(&cont);

        assert_eq!(parsed, cont);
    }

    #[test]
    fn envelope_round_trips_with_empty_cdata() {
        let cont = VmCont::Envelope {
            cdata: empty_control_data(),
            next: Box::new(quit(0)),
        };

        let parsed = round_trip(&cont);

        assert_eq!(parsed, cont);
    }

    #[test]
    fn envelope_round_trips_with_populated_cdata() {
        let cdata = VmControlData {
            nargs: Some(5),
            stack: Some(VmStack(vec![VmStackValue::TinyInt { value: 11 }])),
            save: HashmapE::Empty,
            cp: Some(-1),
        };
        let cont = VmCont::Envelope {
            cdata,
            next: Box::new(VmCont::QuitExc),
        };

        let parsed = round_trip(&cont);

        assert_eq!(parsed, cont);
    }

    #[test]
    fn std_round_trips() {
        let inner = make_data_cell(&[0xab, 0xcd]);
        let code = VmCellSlice {
            cell: inner,
            st_bits: 0,
            end_bits: 16,
            st_ref: 0,
            end_ref: 0,
        };
        let cont = VmCont::Std {
            cdata: empty_control_data(),
            code,
        };

        let parsed = round_trip(&cont);

        assert_eq!(parsed, cont);
    }

    #[test]
    fn nested_cont_round_trips() {
        let inner = VmCont::PushInt {
            value: 1,
            next: Box::new(VmCont::PushInt {
                value: 2,
                next: Box::new(VmCont::PushInt {
                    value: 3,
                    next: Box::new(quit(0)),
                }),
            }),
        };

        let parsed = round_trip(&inner);

        assert_eq!(parsed, inner);
    }

    #[test]
    fn cont_inside_vm_stack_value_round_trips() {
        let cont = VmCont::PushInt {
            value: 99,
            next: Box::new(quit(0)),
        };
        let value = VmStackValue::Cont {
            cont: Box::new(cont),
        };

        let cell = value.to_cell(()).unwrap();
        let parsed = cell.parse_fully::<VmStackValue>(()).unwrap();

        assert_eq!(parsed, value);
    }
}
