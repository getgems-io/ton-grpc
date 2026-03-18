use num_bigint::BigUint;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::bits::NBits;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::{Cell, Error, Ref};
use toner::ton::currency::Grams;
use toner::ton::message::Message;

/// ```tlb
/// msg_import_ext$000 msg:^(Message Any) transaction:^Transaction
///               = InMsg;
/// msg_import_ihr$010 msg:^(Message Any) transaction:^Transaction
///     ihr_fee:Grams proof_created:^Cell = InMsg;
/// msg_import_imm$011 in_msg:^MsgEnvelope
///     transaction:^Transaction fwd_fee:Grams = InMsg;
/// msg_import_fin$100 in_msg:^MsgEnvelope
///     transaction:^Transaction fwd_fee:Grams = InMsg;
/// msg_import_tr$101  in_msg:^MsgEnvelope out_msg:^MsgEnvelope
///     transit_fee:Grams = InMsg;
/// msg_discard_fin$110 in_msg:^MsgEnvelope transaction_id:uint64
///     fwd_fee:Grams = InMsg;
/// msg_discard_tr$111 in_msg:^MsgEnvelope transaction_id:uint64
///     fwd_fee:Grams proof_delivered:^Cell = InMsg;
/// msg_import_deferred_fin$00100 in_msg:^MsgEnvelope
///     transaction:^Transaction fwd_fee:Grams = InMsg;
/// msg_import_deferred_tr$00101 in_msg:^MsgEnvelope out_msg:^MsgEnvelope = InMsg;
/// ```
#[derive(Debug, Clone)]
pub enum InMsg {
    ImportExt {
        msg: Message,
        transaction: Cell,
    },
    ImportIhr {
        msg: Message,
        transaction: Cell,
        ihr_fee: BigUint,
        proof_created: Cell,
    },
    ImportImm {
        in_msg: Cell,
        transaction: Cell,
        fwd_fee: BigUint,
    },
    ImportFin {
        in_msg: Cell,
        transaction: Cell,
        fwd_fee: BigUint,
    },
    ImportTr {
        in_msg: Cell,
        out_msg: Cell,
        transit_fee: BigUint,
    },
    DiscardFin {
        in_msg: Cell,
        transaction_id: u64,
        fwd_fee: BigUint,
    },
    DiscardTr {
        in_msg: Cell,
        transaction_id: u64,
        fwd_fee: BigUint,
        proof_delivered: Cell,
    },
    ImportDeferredFin {
        in_msg: Cell,
        transaction: Cell,
        fwd_fee: BigUint,
    },
    ImportDeferredTr {
        in_msg: Cell,
        out_msg: Cell,
    },
}

impl<'de> CellDeserialize<'de> for InMsg {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u8 = parser.unpack_as::<_, NBits<3>>(())?;

        match tag {
            // msg_import_ext$000
            0b000 => {
                let msg = parser.parse_as::<Message, Ref>(())?;
                let transaction = parser.parse_as::<Cell, Ref>(())?;

                Ok(Self::ImportExt { msg, transaction })
            }
            // msg_import_deferred_fin$00100 or msg_import_deferred_tr$00101
            0b001 => {
                let tag2: u8 = parser.unpack_as::<_, NBits<2>>(())?;

                match tag2 {
                    // msg_import_deferred_fin$00100
                    0b00 => {
                        let in_msg = parser.parse_as::<Cell, Ref>(())?;
                        let transaction = parser.parse_as::<Cell, Ref>(())?;
                        let fwd_fee = parser.unpack_as::<_, Grams>(())?;

                        Ok(Self::ImportDeferredFin {
                            in_msg,
                            transaction,
                            fwd_fee,
                        })
                    }
                    // msg_import_deferred_tr$00101
                    0b01 => {
                        let in_msg = parser.parse_as::<Cell, Ref>(())?;
                        let out_msg = parser.parse_as::<Cell, Ref>(())?;

                        Ok(Self::ImportDeferredTr { in_msg, out_msg })
                    }
                    _ => Err(Error::custom(format!(
                        "invalid InMsg tag: 0b001{:02b}",
                        tag2
                    ))),
                }
            }
            // msg_import_ihr$010
            0b010 => {
                let msg = parser.parse_as::<Message, Ref>(())?;
                let transaction = parser.parse_as::<Cell, Ref>(())?;
                let ihr_fee = parser.unpack_as::<_, Grams>(())?;
                let proof_created = parser.parse_as::<Cell, Ref>(())?;

                Ok(Self::ImportIhr {
                    msg,
                    transaction,
                    ihr_fee,
                    proof_created,
                })
            }
            // msg_import_imm$011
            0b011 => {
                let in_msg = parser.parse_as::<Cell, Ref>(())?;
                let transaction = parser.parse_as::<Cell, Ref>(())?;
                let fwd_fee = parser.unpack_as::<_, Grams>(())?;

                Ok(Self::ImportImm {
                    in_msg,
                    transaction,
                    fwd_fee,
                })
            }
            // msg_import_fin$100
            0b100 => {
                let in_msg = parser.parse_as::<Cell, Ref>(())?;
                let transaction = parser.parse_as::<Cell, Ref>(())?;
                let fwd_fee = parser.unpack_as::<_, Grams>(())?;

                Ok(Self::ImportFin {
                    in_msg,
                    transaction,
                    fwd_fee,
                })
            }
            // msg_import_tr$101
            0b101 => {
                let in_msg = parser.parse_as::<Cell, Ref>(())?;
                let out_msg = parser.parse_as::<Cell, Ref>(())?;
                let transit_fee = parser.unpack_as::<_, Grams>(())?;

                Ok(Self::ImportTr {
                    in_msg,
                    out_msg,
                    transit_fee,
                })
            }
            // msg_discard_fin$110
            0b110 => {
                let in_msg = parser.parse_as::<Cell, Ref>(())?;
                let transaction_id = parser.unpack(())?;
                let fwd_fee = parser.unpack_as::<_, Grams>(())?;

                Ok(Self::DiscardFin {
                    in_msg,
                    transaction_id,
                    fwd_fee,
                })
            }
            // msg_discard_tr$111
            0b111 => {
                let in_msg = parser.parse_as::<Cell, Ref>(())?;
                let transaction_id = parser.unpack(())?;
                let fwd_fee = parser.unpack_as::<_, Grams>(())?;
                let proof_delivered = parser.parse_as::<Cell, Ref>(())?;

                Ok(Self::DiscardTr {
                    in_msg,
                    transaction_id,
                    fwd_fee,
                    proof_delivered,
                })
            }
            _ => Err(Error::custom(format!("invalid InMsg tag: 0b{:03b}", tag))),
        }
    }
}
