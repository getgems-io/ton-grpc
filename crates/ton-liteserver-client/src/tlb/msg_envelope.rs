use crate::tlb::msg_address_int::MsgAddressInt;
use num_bigint::BigUint;
use toner::tlb::bits::NBits;
use toner::ton::currency::Grams;
use toner::ton::message::Message;
use toner_tlb_macros::{BitUnpack, CellDeserialize};

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
#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
pub enum MsgEnvelope {
    #[tlb(tag = "0x4")]
    V1 {
        #[tlb(bits)]
        cur_addr: IntermediateAddress,
        #[tlb(bits)]
        next_addr: IntermediateAddress,
        #[tlb(bits, as = "Grams")]
        fwd_fee_remaining: BigUint,
        #[tlb(cell, as = "toner::tlb::Ref<toner::tlb::ParseFully>")]
        msg: Message,
    },
    #[tlb(tag = "0x5")]
    V2 {
        #[tlb(bits)]
        cur_addr: IntermediateAddress,
        #[tlb(bits)]
        next_addr: IntermediateAddress,
        #[tlb(bits, as = "Grams")]
        fwd_fee_remaining: BigUint,
        #[tlb(cell, as = "toner::tlb::Ref<toner::tlb::ParseFully>")]
        msg: Message,
        #[tlb(bits)]
        emitted_lt: Option<u64>,
        #[tlb(bits)]
        metadata: Option<MsgMetadata>,
    },
}

/// ```tlb
/// interm_addr_regular$0 use_dest_bits:(#<= 96)
///   = IntermediateAddress;
/// interm_addr_simple$10 workchain_id:int8 addr_pfx:uint64
///   = IntermediateAddress;
/// interm_addr_ext$11 workchain_id:int32 addr_pfx:uint64
///   = IntermediateAddress;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, BitUnpack)]
pub enum IntermediateAddress {
    #[tlb(tag = "0b0")]
    Regular {
        #[tlb(bits, as = "NBits<7>")]
        use_dest_bits: u8,
    },
    #[tlb(tag = "0b10")]
    Simple { workchain_id: i8, addr_pfx: u64 },
    #[tlb(tag = "0b11")]
    Ext { workchain_id: i32, addr_pfx: u64 },
}

/// ```tlb
/// msg_metadata#0 depth:uint32 initiator_addr:MsgAddressInt initiator_lt:uint64 = MsgMetadata;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, BitUnpack)]
#[tlb(tag = "0x0")]
pub struct MsgMetadata {
    depth: u32,
    initiator_addr: MsgAddressInt,
    initiator_lt: u64,
}
