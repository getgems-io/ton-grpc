use crate::tlb::in_msg::InMsg;
use crate::tlb::msg_envelope::MsgEnvelope;
use crate::tlb::transaction::Transaction;
use toner::tlb::bits::NBits;
use toner::tlb::{ParseFully, Ref};
use toner::ton::message::Message;
use toner_tlb_macros::CellDeserialize;

#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
#[allow(clippy::enum_variant_names)]
#[allow(clippy::large_enum_variant)] // TODO[akostylev0]: remove this
pub enum OutMsg {
    /// ```tlb
    /// msg_export_ext$000 msg:^(Message Any)
    ///     transaction:^Transaction = OutMsg;
    /// ```
    #[tlb(tag = "0b000")]
    ExportExt {
        #[tlb(parse_as = "Ref<ParseFully>")]
        msg: Message,
        #[tlb(parse_as = "Ref<ParseFully>")]
        transaction: Transaction,
    },
    /// ```tlb
    /// msg_export_imm$010 out_msg:^MsgEnvelope
    ///     transaction:^Transaction reimport:^InMsg = OutMsg;
    /// ```
    #[tlb(tag = "0b010")]
    ExportImm {
        #[tlb(parse_as = "Ref<ParseFully>")]
        out_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        transaction: Transaction,
        #[tlb(parse_as = "Ref<ParseFully>")]
        reimport: InMsg,
    },
    /// ```tlb
    /// msg_export_new$001 out_msg:^MsgEnvelope
    ///     transaction:^Transaction = OutMsg;
    /// ```
    #[tlb(tag = "0b001")]
    ExportNew {
        #[tlb(parse_as = "Ref<ParseFully>")]
        out_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        transaction: Transaction,
    },
    /// ```tlb
    /// msg_export_tr$011 out_msg:^MsgEnvelope
    ///     imported:^InMsg = OutMsg;
    /// ```
    #[tlb(tag = "0b011")]
    ExportTr {
        #[tlb(parse_as = "Ref<ParseFully>")]
        out_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        imported: InMsg,
    },
    /// ```tlb
    /// msg_export_deq$1100 out_msg:^MsgEnvelope
    ///     import_block_lt:uint63 = OutMsg;
    /// ```
    #[tlb(tag = "0b1100")]
    ExportDeq {
        #[tlb(parse_as = "Ref<ParseFully>")]
        out_msg: MsgEnvelope,
        #[tlb(unpack_as = "NBits<63>")]
        import_block_lt: u64,
    },

    /// ```tlb
    /// msg_export_deq_short$1101 msg_env_hash:bits256
    ///     next_workchain:int32 next_addr_pfx:uint64
    ///     import_block_lt:uint64 = OutMsg;
    /// ```
    #[tlb(tag = "0b1101")]
    ExportDeqShort {
        #[tlb(unpack)]
        msg_env_hash: [u8; 32],
        #[tlb(unpack)]
        next_workchain: i32,
        #[tlb(unpack)]
        next_addr_pfx: u64,
        #[tlb(unpack)]
        import_block_lt: u64,
    },
    /// ```tlb
    /// msg_export_tr_req$111 out_msg:^MsgEnvelope
    ///     imported:^InMsg = OutMsg;
    /// ```
    #[tlb(tag = "0b111")]
    ExportTrReq {
        #[tlb(parse_as = "Ref<ParseFully>")]
        out_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        imported: InMsg,
    },
    /// ```tlb
    /// msg_export_deq_imm$100 out_msg:^MsgEnvelope
    ///     reimport:^InMsg = OutMsg;
    /// ```
    #[tlb(tag = "0b100")]
    ExportDeqImm {
        #[tlb(parse_as = "Ref<ParseFully>")]
        out_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        reimport: InMsg,
    },
    /// ```tlb
    /// msg_export_new_defer$10100 out_msg:^MsgEnvelope
    ///     transaction:^Transaction = OutMsg;
    /// ```
    #[tlb(tag = "0b10100")]
    ExportNewDefer {
        #[tlb(parse_as = "Ref<ParseFully>")]
        out_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        transaction: Transaction,
    },
    /// ```tlb
    /// msg_export_deferred_tr$10101 out_msg:^MsgEnvelope
    ///     imported:^InMsg = OutMsg;
    /// ```
    #[tlb(tag = "0b10101")]
    ExportDeferredTr {
        #[tlb(parse_as = "Ref<ParseFully>")]
        out_msg: MsgEnvelope,
        #[tlb(parse_as = "Ref<ParseFully>")]
        imported: InMsg,
    },
}
