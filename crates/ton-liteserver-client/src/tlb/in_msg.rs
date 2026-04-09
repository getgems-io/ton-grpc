use crate::tlb::msg_envelope::MsgEnvelope;
use crate::tlb::transaction::Transaction;
use num_bigint::BigUint;
use toner::tlb::{Cell, ParseFully, Ref};
use toner::ton::currency::Grams;
use toner::ton::message::Message;
use toner_tlb_macros::CellDeserialize;

#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
pub enum InMsg {
    /// ```tlb
    /// msg_import_ext$000 msg:^(Message Any) transaction:^Transaction
    ///               = InMsg;
    /// ```
    #[tlb(tag = "0b000")]
    ImportExt {
        #[tlb(parse_as = "Ref<ParseFully>")]
        msg: Message,
        #[tlb(parse_as = "Ref<ParseFully>")]
        transaction: Transaction,
    },
    /// ```tlb
    /// msg_import_ihr$010 msg:^(Message Any) transaction:^Transaction
    ///     ihr_fee:Grams proof_created:^Cell = InMsg;
    /// ```
    #[tlb(tag = "0b010")]
    ImportIhr {
        #[tlb(parse_as = "Ref<ParseFully>")]
        msg: Message,
        #[tlb(parse_as = "Ref<ParseFully>")]
        transaction: Transaction,
        #[tlb(unpack_as = "Grams")]
        ihr_fee: BigUint,
        #[tlb(parse_as = "Ref<ParseFully>")]
        proof_created: Cell,
    },
    /// ```tlb
    /// msg_import_imm$011 in_msg:^MsgEnvelope
    ///     transaction:^Transaction fwd_fee:Grams = InMsg;
    /// ```
    #[tlb(tag = "0b011")]
    ImportImm {
        #[tlb(parse_as = "Ref<ParseFully>")]
        in_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        transaction: Transaction,
        #[tlb(unpack_as = "Grams")]
        fwd_fee: BigUint,
    },
    /// ```tlb
    /// msg_import_fin$100 in_msg:^MsgEnvelope
    ///     transaction:^Transaction fwd_fee:Grams = InMsg;
    /// ```
    #[tlb(tag = "0b100")]
    ImportFin {
        #[tlb(parse_as = "Ref<ParseFully>")]
        in_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        transaction: Transaction,
        #[tlb(unpack_as = "Grams")]
        fwd_fee: BigUint,
    },
    /// ```tlb
    /// msg_import_tr$101  in_msg:^MsgEnvelope out_msg:^MsgEnvelope
    ///     transit_fee:Grams = InMsg;
    /// ```
    #[tlb(tag = "0b101")]
    ImportTr {
        #[tlb(parse_as = "Ref<ParseFully>")]
        in_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        out_msg: MsgEnvelope,
        #[tlb(unpack_as = "Grams")]
        transit_fee: BigUint,
    },
    /// ```tlb
    /// msg_discard_fin$110 in_msg:^MsgEnvelope transaction_id:uint64
    ///     fwd_fee:Grams = InMsg;
    /// ```
    #[tlb(tag = "0b110")]
    DiscardFin {
        #[tlb(parse_as = "Ref<ParseFully>")]
        in_msg: MsgEnvelope,
        #[tlb(unpack)]
        transaction_id: u64,
        #[tlb(unpack_as = "Grams")]
        fwd_fee: BigUint,
    },
    /// ```tlb
    /// msg_discard_tr$111 in_msg:^MsgEnvelope transaction_id:uint64
    ///     fwd_fee:Grams proof_delivered:^Cell = InMsg;
    /// ```
    #[tlb(tag = "0b111")]
    DiscardTr {
        #[tlb(parse_as = "Ref<ParseFully>")]
        in_msg: MsgEnvelope,
        #[tlb(unpack)]
        transaction_id: u64,
        #[tlb(unpack_as = "Grams")]
        fwd_fee: BigUint,
        #[tlb(parse_as = "Ref<ParseFully>")]
        proof_delivered: Cell,
    },
    /// ```tlb
    /// msg_import_deferred_fin$00100 in_msg:^MsgEnvelope
    ///     transaction:^Transaction fwd_fee:Grams = InMsg;
    /// ```
    #[tlb(tag = "0b00100")]
    ImportDeferredFin {
        #[tlb(parse_as = "Ref<ParseFully>")]
        in_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        transaction: Transaction,
        #[tlb(unpack_as = "Grams")]
        fwd_fee: BigUint,
    },
    /// ```tlb
    /// msg_import_deferred_tr$00101 in_msg:^MsgEnvelope out_msg:^MsgEnvelope = InMsg;
    /// ```
    #[tlb(tag = "0b00101")]
    ImportDeferredTr {
        #[tlb(parse_as = "Ref<ParseFully>")]
        in_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        out_msg: MsgEnvelope,
    },
}
