use crate::tlb::account::Account;
use crate::tlb::account_status::AccountStatus;
use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::hash_update::HashUpdate;
use std::collections::HashMap;
use toner::tlb::bits::bitvec::field::BitField;
use toner::tlb::bits::bitvec::order::Msb0;
use toner::tlb::bits::bitvec::prelude::BitVec;
use toner::tlb::bits::de::BitReaderExt;
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::hashmap::HashmapE;
use toner::tlb::{Cell, ParseFully, Ref};

/// ```tlb
/// transaction$0111 account_addr:bits256 lt:uint64
///   prev_trans_hash:bits256 prev_trans_lt:uint64 now:uint32
///   outmsg_cnt:uint15
///   orig_status:AccountStatus end_status:AccountStatus
///   ^[ in_msg:(Maybe ^(Message Any)) out_msgs:(HashmapE 15 ^(Message Any)) ]
///   total_fees:CurrencyCollection state_update:^(HASH_UPDATE Account)
///   description:^TransactionDescr = Transaction;
/// ```
pub struct Transaction {
    account_addr: [u8; 32],
    lt: u64,
    prev_trans_hash: [u8; 32],
    prev_trans_lt: u64,
    now: u32,
    outmsg_cnt: u16,
    orig_status: AccountStatus,
    end_status: AccountStatus,
    in_msg: Option<Cell>,         // TODO[akostylev0]: Message
    out_msgs: HashMap<u16, Cell>, // TODO[akostylev0]: Message
    total_fees: CurrencyCollection,
    state_update: HashUpdate<Account>,
    description: Cell, // TODO[akostylev0]: TransactionDescr
}

impl<'de> CellDeserialize<'de> for Transaction {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let account_addr = parser.unpack(())?;
        let lt = parser.unpack(())?;
        let prev_trans_hash = parser.unpack(())?;
        let prev_trans_lt = parser.unpack(())?;
        let now = parser.unpack(())?;
        let outmsg_cnt = parser.unpack(())?;
        let orig_status = parser.unpack(())?;
        let end_status = parser.unpack(())?;

        // TODO[akostylev0]: parse as Key
        let (in_msg, out_msgs): (_, HashMap<BitVec<u8, Msb0>, Cell>) =
            parser.parse_as::<_, Ref<ParseFully<(Option<Ref>, HashmapE<Ref>)>>>(((), (15, ())))?;

        let out_msgs = out_msgs
            .into_iter()
            .map(|(k, v): (BitVec<u8, Msb0>, Cell)| (k.load_be::<u16>(), v))
            .collect();

        let total_fees = parser.parse(())?;
        let state_update = parser.unpack(())?;
        let description = parser.parse_as::<_, Ref>(())?;

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
