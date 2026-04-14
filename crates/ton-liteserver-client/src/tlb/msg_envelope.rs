use crate::tlb::msg_address_int::MsgAddressInt;
use num_bigint::BigUint;
use toner::tlb::ParseFully;
use toner::tlb::bits::NBits;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::tlb::de::CellDeserialize;
use toner::tlb::de::{CellParser, CellParserError};
use toner::tlb::{Context, Error, Ref};
use toner::ton::currency::Grams;
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
        let tag: u8 = parser.unpack_as::<_, NBits<4>>(()).context("tag")?;
        let msg_envelope = match tag {
            0x4 => {
                let cur_addr = parser.unpack(()).context("v1 cur_addr")?;
                let next_addr = parser.unpack(()).context("v1 next_addr")?;
                let fwd_fee_remaining = parser
                    .unpack_as::<_, Grams>(())
                    .context("v1 fwd_fee_remaining")?;
                let msg = parser
                    .parse_as::<_, Ref<ParseFully>>(())
                    .context("v1 msg")?;

                MsgEnvelope::V1 {
                    cur_addr,
                    next_addr,
                    fwd_fee_remaining,
                    msg,
                }
            }
            0x5 => {
                let cur_addr = parser.unpack(()).context("v2 cur_addr")?;
                let next_addr = parser.unpack(()).context("v2 next_addr")?;
                let fwd_fee_remaining = parser
                    .unpack_as::<_, Grams>(())
                    .context("v2 fwd_fee_remaining")?;
                let msg = parser
                    .parse_as::<_, Ref<ParseFully>>(())
                    .context("v2 msg")?;
                let emitted_lt = parser.unpack(()).context("v2 emitted_lt")?;
                let metadata = parser.unpack(()).context("v2 metadata")?;

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
    Regular { use_dest_bits: u8 },
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
                use_dest_bits: reader
                    .unpack_as::<_, NBits<7>>(())
                    .context("regular use_dest_bits")?,
            }),
            Some(true) => match reader.read_bit()? {
                None => Err(Error::custom("not enough bits")),
                Some(false) => Ok(IntermediateAddress::Simple {
                    workchain_id: reader.unpack(()).context("workchain_id")?,
                    addr_pfx: reader.unpack(()).context("addr_pfx")?,
                }),
                Some(true) => Ok(IntermediateAddress::Ext {
                    workchain_id: reader.unpack(()).context("simple workchain_id")?,
                    addr_pfx: reader.unpack(()).context("ext addr_pfx")?,
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
