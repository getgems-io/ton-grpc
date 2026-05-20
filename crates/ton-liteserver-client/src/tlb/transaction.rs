use crate::tlb::account::Account;
use crate::tlb::account_status::AccountStatus;
use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::hash_update::HashUpdate;
use crate::tlb::transaction_descr::TransactionDescr;
use std::collections::HashMap;
use toner::tlb::bits::NBits;
use toner::tlb::bits::bitvec::field::BitField;
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::vec::BitVec;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::hashmap::HashmapE;
use toner::tlb::{Context, Data, Error, ParseFully, Ref};
use toner::ton::message::Message;

/// ```tlb
/// transaction$0111 account_addr:bits256 lt:uint64
///   prev_trans_hash:bits256 prev_trans_lt:uint64 now:uint32
///   outmsg_cnt:uint15
///   orig_status:AccountStatus end_status:AccountStatus
///   ^[ in_msg:(Maybe ^(Message Any)) out_msgs:(HashmapE 15 ^(Message Any)) ]
///   total_fees:CurrencyCollection state_update:^(HASH_UPDATE Account)
///   description:^TransactionDescr = Transaction;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub account_addr: [u8; 32],
    pub lt: u64,
    pub prev_trans_hash: [u8; 32],
    pub prev_trans_lt: u64,
    pub now: u32,
    pub outmsg_cnt: u16,
    pub orig_status: AccountStatus,
    pub end_status: AccountStatus,
    pub in_msg: Option<Message>,
    pub out_msgs: HashMap<u16, Message>,
    pub total_fees: CurrencyCollection,
    pub state_update: HashUpdate<Account>,
    pub description: TransactionDescr,
}

impl<'de> CellDeserialize<'de> for Transaction {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: u8 = parser.unpack_as::<_, NBits<4>>(()).context("tag")?;
        if tag != 0b0111 {
            return Err(Error::custom(format!("invalid tag: {:b}", tag)));
        }

        let account_addr = parser.unpack(()).context("account_addr")?;
        let lt = parser.unpack(()).context("lt")?;
        let prev_trans_hash = parser.unpack(()).context("prev_trans_hash")?;
        let prev_trans_lt = parser.unpack(()).context("prev_trans_lt")?;
        let now = parser.unpack(()).context("now")?;
        let outmsg_cnt = parser.unpack_as::<_, NBits<15>>(()).context("outmsg_cnt")?;
        let orig_status = parser.unpack(()).context("orig_status")?;
        let end_status = parser.unpack(()).context("end_status")?;

        // TODO[akostylev0]: parse as Key
        let (in_msg, out_msgs): (_, HashMap<BitVec<u8, Msb0>, _>) = parser
            .parse_as::<_, Ref<ParseFully<(Option<Ref>, HashmapE<Ref>)>>>(((), (15, ())))
            .context("(in_msg, out_msgs)")?;

        let out_msgs = out_msgs
            .into_iter()
            .map(|(k, v): (BitVec<u8, Msb0>, _)| (k.load_be::<u16>(), v))
            .collect();

        let total_fees = parser.parse(()).context("total_fees")?;
        let state_update = parser
            .parse_as::<_, Ref<ParseFully<Data>>>(())
            .context("state_update")?;
        let description = parser.parse_as::<_, Ref>(()).context("description")?;

        Ok(Self {
            account_addr,
            lt,
            prev_trans_hash,
            prev_trans_lt,
            now,
            outmsg_cnt,
            orig_status,
            end_status,
            in_msg,
            out_msgs,
            total_fees,
            state_update,
            description,
        })
    }
}
