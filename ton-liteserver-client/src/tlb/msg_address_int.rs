use adnl_tcp::types::Int256;
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::vec::BitVec;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::tlb::bits::NBits;
use toner::ton::Anycast;

/// ```tlb
/// addr_std$10 anycast:(Maybe Anycast)
///    workchain_id:int8 address:bits256  = MsgAddressInt;
/// addr_var$11 anycast:(Maybe Anycast) addr_len:(## 9)
///    workchain_id:int32 address:(bits addr_len) = MsgAddressInt;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MsgAddressInt {
    Std {
        anycast: Option<Anycast>,
        workchain_id: i8,
        address: Int256,
    },
    Var {
        anycast: Option<Anycast>,
        workchain_id: i32,
        address: BitVec<u8, Msb0>,
    },
}

impl<'de> BitUnpack<'de> for MsgAddressInt {
    type Args = ();

    fn unpack<R>(reader: &mut R, args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let tag: u8 = reader.unpack_as::<_, NBits<2>>(())?;
        match tag {
            0b10 => {
                let anycast = reader.unpack(())?;
                let workchain_id = reader.unpack(())?;
                let address = reader.unpack(())?;

                Ok(MsgAddressInt::Std {
                    anycast,
                    workchain_id,
                    address,
                })
            }
            0b11 => {
                let anycast = reader.unpack(())?;
                let addr_len = reader.unpack_as::<usize, NBits<9>>(())?;
                let workchain_id = reader.unpack(())?;
                let address = reader.unpack(addr_len * 8)?;
                println!("bits_lieft = {}", reader.bits_left());

                Ok(MsgAddressInt::Var {
                    anycast,
                    workchain_id,
                    address,
                })
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tlb::msg_address_int::MsgAddressInt;
    use toner::tlb::bits::bitvec::{bits, bitvec};
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::de::unpack_fully;

    #[test]
    pub fn msg_address_int_std() {
        let mut input = bitvec![u8, Msb0; 1, 0, 0];
        input.extend_from_raw_slice(&[0; 33]);

        let actual: MsgAddressInt = unpack_fully(&input, ()).unwrap();

        assert_eq!(
            actual,
            MsgAddressInt::Std {
                anycast: None,
                workchain_id: 0,
                address: [0; 32]
            }
        );
    }

    #[test]
    pub fn msg_address_int_var() {
        // TODO[akostylev0]: verify on real data
        let mut input = bitvec![u8, Msb0; 1, 1, 0];
        input.extend_from_bitslice(bits![u8, Msb0; 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        input.extend_from_raw_slice(&[1; 4]);
        input.extend_from_raw_slice(&[1; 1]);

        let actual: MsgAddressInt = unpack_fully(&input, ()).unwrap();

        assert_eq!(
            actual,
            MsgAddressInt::Var {
                anycast: None,
                workchain_id: 16843009,
                address: bitvec![u8, Msb0; 0, 0, 0, 0, 0, 0, 0, 1]
            }
        );
    }
}
