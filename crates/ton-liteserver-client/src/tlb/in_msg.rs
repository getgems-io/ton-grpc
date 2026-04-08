use crate::tlb::msg_envelope::MsgEnvelope;
use crate::tlb::transaction::Transaction;
use num_bigint::BigUint;
use toner::tlb::bits::NBits;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::{Cell, Context, Error, ParseFully, Ref};
use toner::ton::currency::Grams;
use toner::ton::message::Message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InMsg {
    /// ```tlb
    /// msg_import_ext$000 msg:^(Message Any) transaction:^Transaction
    ///               = InMsg;
    /// ```
    ImportExt {
        msg: Message,
        transaction: Transaction,
    },
    /// ```tlb
    /// msg_import_ihr$010 msg:^(Message Any) transaction:^Transaction
    ///     ihr_fee:Grams proof_created:^Cell = InMsg;
    /// ```
    ImportIhr {
        msg: Message,
        transaction: Transaction,
        ihr_fee: BigUint,
        proof_created: Cell,
    },
    /// ```tlb
    /// msg_import_imm$011 in_msg:^MsgEnvelope
    ///     transaction:^Transaction fwd_fee:Grams = InMsg;
    /// ```
    ImportImm {
        in_msg: MsgEnvelope,
        transaction: Transaction,
        fwd_fee: BigUint,
    },
    /// ```tlb
    /// msg_import_fin$100 in_msg:^MsgEnvelope
    ///     transaction:^Transaction fwd_fee:Grams = InMsg;
    /// ```
    ImportFin {
        in_msg: MsgEnvelope,
        transaction: Transaction,
        fwd_fee: BigUint,
    },
    /// ```tlb
    /// msg_import_tr$101  in_msg:^MsgEnvelope out_msg:^MsgEnvelope
    ///     transit_fee:Grams = InMsg;
    /// ```
    ImportTr {
        in_msg: MsgEnvelope,
        out_msg: MsgEnvelope,
        transit_fee: BigUint,
    },
    /// ```tlb
    /// msg_discard_fin$110 in_msg:^MsgEnvelope transaction_id:uint64
    ///     fwd_fee:Grams = InMsg;
    /// ```
    DiscardFin {
        in_msg: MsgEnvelope,
        transaction_id: u64,
        fwd_fee: BigUint,
    },
    /// ```tlb
    /// msg_discard_tr$111 in_msg:^MsgEnvelope transaction_id:uint64
    ///     fwd_fee:Grams proof_delivered:^Cell = InMsg;
    /// ```
    DiscardTr {
        in_msg: MsgEnvelope,
        transaction_id: u64,
        fwd_fee: BigUint,
        proof_delivered: Cell,
    },
    /// ```tlb
    /// msg_import_deferred_fin$00100 in_msg:^MsgEnvelope
    ///     transaction:^Transaction fwd_fee:Grams = InMsg;
    /// ```
    ImportDeferredFin {
        in_msg: MsgEnvelope,
        transaction: Transaction,
        fwd_fee: BigUint,
    },
    /// ```tlb
    /// msg_import_deferred_tr$00101 in_msg:^MsgEnvelope out_msg:^MsgEnvelope = InMsg;
    /// ```
    ImportDeferredTr {
        in_msg: MsgEnvelope,
        out_msg: MsgEnvelope,
    },
}

impl<'de> CellDeserialize<'de> for InMsg {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u8 = parser.unpack_as::<_, NBits<3>>(())?;
        Ok(match tag {
            0b000 => Self::ImportExt {
                msg: parser
                    .parse_as::<_, Ref<ParseFully>>(())
                    .context("msg_import_ext msg")?,
                transaction: parser
                    .parse_as::<_, Ref<ParseFully>>(())
                    .context("msg_import_ext transaction")?,
            },
            0b010 => Self::ImportIhr {
                msg: parser
                    .parse_as::<_, Ref<ParseFully>>(())
                    .context("msg_import_ihr msg")?,
                transaction: parser
                    .parse_as::<_, Ref<ParseFully>>(())
                    .context("msg_import_ihr transaction")?,
                ihr_fee: parser
                    .unpack_as::<_, Grams>(())
                    .context("msg_import_ihr ihr_fee")?,
                proof_created: parser
                    .parse_as::<_, Ref<ParseFully>>(())
                    .context("msg_import_ihr proof_created")?,
            },
            0b011 => Self::ImportImm {
                in_msg: parser
                    .parse_as::<_, Ref<ParseFully>>(())
                    .context("msg_import_imm msg")?,
                transaction: parser
                    .parse_as::<_, Ref<ParseFully>>(())
                    .context("msg_import_imm transaction")?,
                fwd_fee: parser
                    .unpack_as::<_, Grams>(())
                    .context("msg_import_imm fwd_fee")?,
            },
            0b100 => Self::ImportFin {
                in_msg: parser.parse_as::<_, Ref<ParseFully>>(())?,
                transaction: parser.parse_as::<_, Ref<ParseFully>>(())?,
                fwd_fee: parser.unpack_as::<_, Grams>(())?,
            },
            0b101 => Self::ImportTr {
                in_msg: parser.parse_as::<_, Ref<ParseFully>>(())?,
                out_msg: parser.parse_as::<_, Ref<ParseFully>>(())?,
                transit_fee: parser.unpack_as::<_, Grams>(())?,
            },
            0b110 => Self::DiscardFin {
                in_msg: parser.parse_as::<_, Ref<ParseFully>>(())?,
                transaction_id: parser.unpack(())?,
                fwd_fee: parser.unpack_as::<_, Grams>(())?,
            },
            0b111 => Self::DiscardTr {
                in_msg: parser.parse_as::<_, Ref<ParseFully>>(())?,
                transaction_id: parser.unpack(())?,
                fwd_fee: parser.unpack_as::<_, Grams>(())?,
                proof_delivered: parser.parse_as::<_, Ref<ParseFully>>(())?,
            },
            0b001 => {
                let tag: u8 = parser.unpack_as::<_, NBits<2>>(())?;
                match tag {
                    0b00 => Self::ImportDeferredFin {
                        in_msg: parser.parse_as::<_, Ref<ParseFully>>(())?,
                        transaction: parser.parse_as::<_, Ref<ParseFully>>(())?,
                        fwd_fee: parser.unpack_as::<_, Grams>(())?,
                    },
                    0b01 => Self::ImportDeferredTr {
                        in_msg: parser.parse_as::<_, Ref<ParseFully>>(())?,
                        out_msg: parser.parse_as::<_, Ref<ParseFully>>(())?,
                    },
                    _ => return Err(Error::custom(format!("invalid InMsg tag: {}", tag))),
                }
            }
            _ => return Err(Error::custom(format!("invalid InMsg tag: {}", tag))),
        })
    }
}
