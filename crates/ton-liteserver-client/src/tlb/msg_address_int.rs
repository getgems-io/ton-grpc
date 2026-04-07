use toner::tlb::Error;
use toner::tlb::bits::NBits;
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::vec::BitVec;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::ton::Anycast;

/// ```tlb
/// addr_std$10 anycast:(Maybe Anycast)
///    workchain_id:int8 address:bits256  = MsgAddressInt;
/// addr_var$11 anycast:(Maybe Anycast) addr_len:(## 9)
///    workchain_id:int32 address:(bits addr_len) = MsgAddressInt;
/// ```
#[derive(Debug, Clone, PartialEq)]
// TODO[akostylev0]: impl traits on Anycast
pub enum MsgAddressInt {
    Std {
        // anycast: Option<Anycast>,
        workchain_id: i8,
        address: [u8; 32],
    },
    Var {
        // anycast: Option<Anycast>,
        workchain_id: i32,
        address: BitVec<u8, Msb0>,
    },
}

impl<'de> BitUnpack<'de> for MsgAddressInt {
    type Args = ();

    #[inline]
    fn unpack<R>(reader: &mut R, _: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        match reader.unpack_as::<u8, NBits<2>>(())? {
            0b10 => {
                let _anycast: Option<Anycast> = reader.unpack(())?;
                Ok(Self::Std {
                    // anycast,
                    workchain_id: reader.unpack(())?,
                    address: reader.unpack(())?,
                })
            }
            0b11 => {
                let _anycast: Option<Anycast> = reader.unpack(())?;
                let addr_len: usize = reader.unpack_as::<_, NBits<9>>(())?;

                Ok(Self::Var {
                    // anycast,
                    workchain_id: reader.unpack(())?,
                    address: reader.unpack(addr_len)?,
                })
            }
            tag => Err(Error::custom(format!(
                "unsupported MsgAddressInt tag: {tag}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tlb::msg_address_int::MsgAddressInt;
    use toner::tlb::bits::bitvec::bitvec;
    use toner::tlb::bits::bitvec::order::Msb0;
    use toner::tlb::bits::bitvec::view::BitView;
    use toner::tlb::bits::de::unpack_fully;

    #[test]
    fn unpack_std() {
        let mut bits = bitvec![u8, Msb0; 1, 0]; // std tag
        bits.push(false); // no anycast
        bits.extend(255u8.view_bits::<Msb0>()); // workchain_id
        bits.extend([1u8; 32].view_bits::<Msb0>()); // address

        let actual: MsgAddressInt = unpack_fully(&bits, ()).unwrap();

        assert_eq!(
            actual,
            MsgAddressInt::Std {
                // anycast: None,
                workchain_id: -1,
                address: [1u8; 32],
            }
        )
    }

    #[test]
    fn unpack_var() {
        let mut bits = bitvec![u8, Msb0; 1, 1]; // var tag
        bits.push(false); // no anycast
        bits.push(true); // addr_len
        bits.extend(0u8.view_bits::<Msb0>());
        bits.extend(1u32.view_bits::<Msb0>()); // workchain_id
        bits.extend([1u8; 32].view_bits::<Msb0>()); // address

        let actual: MsgAddressInt = unpack_fully(&bits, ()).unwrap();

        assert_eq!(
            actual,
            MsgAddressInt::Var {
                // anycast: None,
                workchain_id: 1,
                address: [1u8; 32].view_bits().to_bitvec(),
            }
        )
    }
}
