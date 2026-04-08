use crate::tlb::msg_address_int::MsgAddressInt;
use num_bigint::BigUint;
use toner::tlb::bits::VarLen;
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::prelude::BitVec;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::tlb::de::{CellParser, CellParserError};
use toner::tlb::{Error, Ref};
use toner::ton::ParseFully;
use toner::ton::bits::NBits;
use toner::ton::currency::Grams;
use toner::ton::de::CellDeserialize;
use toner::ton::message::Message;

/// ```tlb
/// msg_envelope#4 cur_addr:IntermediateAddress
///   next_addr:IntermediateAddress fwd_fee_remaining:Grams
///   msg:^(Message Any) = MsgEnvelope;
/// msg_envelope_v2#5 cur_addr:IntermediateAddress
///   next_addr:IntermediateAddress fwd_fee_remaining:Grams
///   msg:^(Message Any)
///   emitted_lt:(Maybe uint64)
///   metadata:(Maybe MsgMetadata) = MsgEnvelope;
/// ```
pub enum MsgEnvelope {
    V1 {
        cur_addr: IntermediateAddress,
        next_addr: IntermediateAddress,
        fwd_fee_remaining: BigUint,
        msg: Message,
    },
    V2 {
        cur_addr: IntermediateAddress,
        next_addr: IntermediateAddress,
        fwd_fee_remaining: BigUint,
        msg: Message,
        emitted_lt: Option<u64>,
        metadata: Option<MsgMetadata>,
    },
}

impl<'de> CellDeserialize<'de> for MsgEnvelope {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u8 = parser.unpack_as::<_, NBits<4>>(())?;
        let msg_envelope = match tag {
            0x4 => {
                let cur_addr = parser.unpack(())?;
                let next_addr = parser.unpack(())?;
                let fwd_fee_remaining = parser.unpack_as::<_, Grams>(())?;
                let msg = parser.parse_as::<_, Ref<ParseFully>>(())?;

                MsgEnvelope::V1 {
                    cur_addr,
                    next_addr,
                    fwd_fee_remaining,
                    msg,
                }
            }
            0x5 => {
                let cur_addr = parser.unpack(())?;
                let next_addr = parser.unpack(())?;
                let fwd_fee_remaining = parser.unpack_as::<_, Grams>(())?;
                let msg = parser.parse_as::<_, Ref<ParseFully>>(())?;
                let emitted_lt = parser.unpack(())?;
                let metadata = parser.unpack(())?;

                MsgEnvelope::V2 {
                    cur_addr,
                    next_addr,
                    fwd_fee_remaining,
                    msg,
                    emitted_lt,
                    metadata,
                }
            }
            _ => return Err(Error::custom(format!("unsupported MsgEnvelope tag: {tag}"))),
        };

        parser.ensure_empty()?;

        Ok(msg_envelope)
    }
}

/// ```tlb
/// interm_addr_regular$0 use_dest_bits:(#<= 96)
///   = IntermediateAddress;
/// interm_addr_simple$10 workchain_id:int8 addr_pfx:uint64
///   = IntermediateAddress;
/// interm_addr_ext$11 workchain_id:int32 addr_pfx:uint64
///   = IntermediateAddress;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IntermediateAddress {
    Regular { use_dest_bits: BitVec<u8, Msb0> },
    Simple { workchain_id: i8, addr_pfx: u64 },
    Ext { workchain_id: i32, addr_pfx: u64 },
}

impl<'de> BitUnpack<'de> for IntermediateAddress {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        match reader.read_bit()? {
            None => Err(Error::custom("not enough bits")),
            Some(false) => Ok(IntermediateAddress::Regular {
                use_dest_bits: reader.unpack_as::<_, VarLen<_, 7>>(())?,
            }),
            Some(true) => match reader.read_bit()? {
                None => Err(Error::custom("not enough bits")),
                Some(false) => Ok(IntermediateAddress::Simple {
                    workchain_id: reader.unpack(())?,
                    addr_pfx: reader.unpack(())?,
                }),
                Some(true) => Ok(IntermediateAddress::Ext {
                    workchain_id: reader.unpack(())?,
                    addr_pfx: reader.unpack(())?,
                }),
            },
        }
    }
}

/// ```tlb
/// msg_metadata#0 depth:uint32 initiator_addr:MsgAddressInt initiator_lt:uint64 = MsgMetadata;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsgMetadata {
    depth: u32,
    initiator_addr: MsgAddressInt,
    initiator_lt: u64,
}

impl<'de> BitUnpack<'de> for MsgMetadata {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let tag: u8 = reader.unpack_as::<_, NBits<4>>(())?;
        if tag != 0 {
            return Err(Error::custom(format!("unsupported MsgMetadata tag: {tag}")));
        }

        let depth: u32 = reader.unpack(())?;
        let initiator_addr = reader.unpack(())?;
        let initiator_lt: u64 = reader.unpack(())?;

        Ok(MsgMetadata {
            depth,
            initiator_addr,
            initiator_lt,
        })
    }
}
