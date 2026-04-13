use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::storage_used::StorageUsed;
use crate::tlb::transaction::Transaction;
use num_bigint::BigUint;
use toner::tlb::bits::de::{BitReader, BitReaderExt, BitUnpack};
use toner::tlb::bits::{NBits, VarInt};
use toner::tlb::de::{CellDeserialize, CellParser, CellParserError};
use toner::tlb::{Data, Error, ParseFully, Ref};
use toner::ton::currency::Grams;
use toner_tlb_macros::CellDeserialize;

#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
pub enum TransactionDescr {
    /// ```tlb
    /// trans_ord$0000 credit_first:Bool
    ///   storage_ph:(Maybe TrStoragePhase)
    ///   credit_ph:(Maybe TrCreditPhase)
    ///   compute_ph:TrComputePhase action:(Maybe ^TrActionPhase)
    ///   aborted:Bool bounce:(Maybe TrBouncePhase)
    ///   destroyed:Bool
    ///   = TransactionDescr;
    /// ```
    #[tlb(tag = "$0000")]
    Ordinary {
        #[tlb(unpack)]
        credit_first: bool,
        #[tlb(unpack)]
        storage_ph: Option<TrStoragePhase>,
        credit_ph: Option<TrCreditPhase>,
        compute_ph: TrComputePhase,
        #[tlb(parse_as = "Option<Ref<ParseFully<Data>>>")]
        action: Option<TrActionPhase>,
        #[tlb(unpack)]
        aborted: bool,
        #[tlb(unpack)]
        bounce: Option<TrBouncePhase>,
        #[tlb(unpack)]
        destroyed: bool,
    },
    /// ```tlb
    /// trans_storage$0001 storage_ph:TrStoragePhase
    ///   = TransactionDescr;
    /// ```
    #[tlb(tag = "$0001")]
    Storage {
        #[tlb(unpack)]
        storage_ph: TrStoragePhase,
    },
    /// ```tlb
    /// trans_tick_tock$001 is_tock:Bool storage_ph:TrStoragePhase
    ///   compute_ph:TrComputePhase action:(Maybe ^TrActionPhase)
    ///   aborted:Bool destroyed:Bool = TransactionDescr;
    /// ```
    #[tlb(tag = "$001")]
    TickTock {
        #[tlb(unpack)]
        is_tock: bool,
        #[tlb(unpack)]
        storage_ph: TrStoragePhase,
        compute_ph: TrComputePhase,
        #[tlb(parse_as = "Option<Ref<ParseFully<Data>>>")]
        action: Option<TrActionPhase>,
        #[tlb(unpack)]
        aborted: bool,
        #[tlb(unpack)]
        destroyed: bool,
    },
    /// ```tlb
    /// trans_split_prepare$0100 split_info:SplitMergeInfo
    ///   storage_ph:(Maybe TrStoragePhase)
    ///   compute_ph:TrComputePhase action:(Maybe ^TrActionPhase)
    ///   aborted:Bool destroyed:Bool
    ///   = TransactionDescr;
    /// ```
    #[tlb(tag = "$0100")]
    SplitPrepare {
        #[tlb(unpack)]
        split_info: SplitMergeInfo,
        #[tlb(unpack)]
        storage_ph: Option<TrStoragePhase>,
        compute_ph: TrComputePhase,
        #[tlb(parse_as = "Option<Ref<ParseFully<Data>>>")]
        action: Option<TrActionPhase>,
        #[tlb(unpack)]
        aborted: bool,
        #[tlb(unpack)]
        destroyed: bool,
    },
    /// ```tlb
    /// trans_split_install$0101 split_info:SplitMergeInfo
    ///   prepare_transaction:^Transaction
    ///   installed:Bool = TransactionDescr;
    /// ```
    #[tlb(tag = "$0101")]
    SplitInstall {
        #[tlb(unpack)]
        split_info: SplitMergeInfo,
        #[tlb(parse_as = "Ref")]
        prepare_transaction: Box<Transaction>,
        #[tlb(unpack)]
        installed: bool,
    },
    /// ```tlb
    /// trans_merge_prepare$0110 split_info:SplitMergeInfo
    ///   storage_ph:TrStoragePhase aborted:Bool
    ///   = TransactionDescr;
    /// ```
    #[tlb(tag = "$0110")]
    MergePrepare {
        #[tlb(unpack)]
        split_info: SplitMergeInfo,
        #[tlb(unpack)]
        storage_ph: TrStoragePhase,
        #[tlb(unpack)]
        aborted: bool,
    },
    /// ```tlb
    /// trans_merge_install$0111 split_info:SplitMergeInfo
    ///   prepare_transaction:^Transaction
    ///   storage_ph:(Maybe TrStoragePhase)
    ///   credit_ph:(Maybe TrCreditPhase)
    ///   compute_ph:TrComputePhase action:(Maybe ^TrActionPhase)
    ///   aborted:Bool destroyed:Bool
    ///   = TransactionDescr;
    /// ```
    #[tlb(tag = "$0111")]
    MergeInstall {
        #[tlb(unpack)]
        split_info: SplitMergeInfo,
        #[tlb(parse_as = "Ref")]
        prepare_transaction: Box<Transaction>,
        #[tlb(unpack)]
        storage_ph: Option<TrStoragePhase>,
        credit_ph: Option<TrCreditPhase>,
        compute_ph: TrComputePhase,
        #[tlb(parse_as = "Option<Ref<ParseFully<Data>>>")]
        action: Option<TrActionPhase>,
        #[tlb(unpack)]
        aborted: bool,
        #[tlb(unpack)]
        destroyed: bool,
    },
}

/// ```tlb
/// tr_phase_storage$_ storage_fees_collected:Grams
///   storage_fees_due:(Maybe Grams)
///   status_change:AccStatusChange
///   = TrStoragePhase;
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrStoragePhase {
    storage_fees_collected: BigUint,
    storage_fees_due: Option<BigUint>,
    status_change: AccStatusChange,
}

impl<'de> BitUnpack<'de> for TrStoragePhase {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        Ok(Self {
            storage_fees_collected: reader.unpack_as::<_, Grams>(())?,
            storage_fees_due: reader.unpack_as::<_, Option<Grams>>(())?,
            status_change: reader.unpack(())?,
        })
    }
}

/// ```tlb
/// tr_phase_credit$_ due_fees_collected:(Maybe Grams)
///   credit:CurrencyCollection = TrCreditPhase;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, CellDeserialize)]
pub struct TrCreditPhase {
    #[tlb(unpack_as = "Option<Grams>")]
    due_fees_collected: Option<BigUint>,
    credit: CurrencyCollection,
}

/// ```tlb
/// acst_unchanged$0 = AccStatusChange;  // x -> x
/// acst_frozen$10 = AccStatusChange;    // init -> frozen
/// acst_deleted$11 = AccStatusChange;   // frozen -> deleted
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AccStatusChange {
    Unchanged,
    Frozen,
    Deleted,
}

impl<'de> BitUnpack<'de> for AccStatusChange {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let bit: bool = reader.unpack(())?;
        if !bit {
            return Ok(AccStatusChange::Unchanged);
        }

        let bit: bool = reader.unpack(())?;

        match bit {
            true => Ok(AccStatusChange::Deleted),
            false => Ok(AccStatusChange::Frozen),
        }
    }
}

/// ```tlb
/// cskip_no_state$00 = ComputeSkipReason;
/// cskip_bad_state$01 = ComputeSkipReason;
/// cskip_no_gas$10 = ComputeSkipReason;
/// cskip_suspended$110 = ComputeSkipReason;
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ComputeSkipReason {
    NoState,
    BadState,
    NoGas,
    Suspended,
}

impl<'de> BitUnpack<'de> for ComputeSkipReason {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let bits = reader.unpack_as::<_, NBits<2>>(())?;
        match bits {
            0b00 => Ok(ComputeSkipReason::NoState),
            0b01 => Ok(ComputeSkipReason::BadState),
            0b10 => Ok(ComputeSkipReason::NoGas),
            0b11 => {
                let bit: bool = reader.unpack(())?;
                if bit {
                    return Err(Error::custom("invalid tag"));
                }

                Ok(ComputeSkipReason::Suspended)
            }
            _ => unreachable!(),
        }
    }
}

/// ```tlb
/// tr_phase_compute_skipped$0 reason:ComputeSkipReason
///   = TrComputePhase;
/// tr_phase_compute_vm$1 success:Bool msg_state_used:Bool
///   account_activated:Bool gas_fees:Grams
///   ^[ gas_used:(VarUInteger 7)
///   gas_limit:(VarUInteger 7) gas_credit:(Maybe (VarUInteger 3))
///   mode:int8 exit_code:int32 exit_arg:(Maybe int32)
///   vm_steps:uint32
///   vm_init_state_hash:bits256 vm_final_state_hash:bits256 ]
///   = TrComputePhase;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrComputePhase {
    Skipped {
        reason: ComputeSkipReason,
    },
    Vm {
        success: bool,
        msg_state_used: bool,
        account_activated: bool,
        gas_fees: BigUint,
        gas_used: BigUint,
        gas_limit: BigUint,
        gas_credit: Option<BigUint>,
        mode: i8,
        exit_code: i32,
        exit_arg: Option<i32>,
        vm_steps: u32,
        vm_init_state_hash: [u8; 32],
        vm_final_state_hash: [u8; 32],
    },
}

impl<'de> CellDeserialize<'de> for TrComputePhase {
    type Args = ();

    fn parse(
        parser: &mut CellParser<'de>,
        _args: Self::Args,
    ) -> Result<Self, CellParserError<'de>> {
        let tag: bool = parser.unpack(())?;
        match tag {
            false => Ok(Self::Skipped {
                reason: parser.unpack(())?,
            }),
            true => {
                let success = parser.unpack(())?;
                let msg_state_used = parser.unpack(())?;
                let account_activated = parser.unpack(())?;
                let gas_fees = parser.unpack_as::<_, Grams>(())?;
                let data: TrComputePhaseVmInnerCell = parser.parse_as::<_, Ref>(())?;

                Ok(Self::Vm {
                    success,
                    msg_state_used,
                    account_activated,
                    gas_fees,
                    gas_used: data.gas_used,
                    gas_limit: data.gas_limit,
                    gas_credit: data.gas_credit,
                    mode: data.mode,
                    exit_code: data.exit_code,
                    exit_arg: data.exit_arg,
                    vm_steps: data.vm_steps,
                    vm_init_state_hash: data.vm_init_state_hash,
                    vm_final_state_hash: data.vm_final_state_hash,
                })
            }
        }
    }
}

/// ```tlb
///   ^[ gas_used:(VarUInteger 7)
///   gas_limit:(VarUInteger 7) gas_credit:(Maybe (VarUInteger 3))
///   mode:int8 exit_code:int32 exit_arg:(Maybe int32)
///   vm_steps:uint32
///   vm_init_state_hash:bits256 vm_final_state_hash:bits256 ]
/// ```
#[derive(CellDeserialize)]
#[tlb(ensure_empty)]
// TODO[akostylev0] implement BitUnpack
struct TrComputePhaseVmInnerCell {
    #[tlb(unpack_as = "VarInt<3>")]
    gas_used: BigUint,
    #[tlb(unpack_as = "VarInt<3>")]
    gas_limit: BigUint,
    #[tlb(unpack_as = "Option<VarInt<2>>")]
    gas_credit: Option<BigUint>,
    #[tlb(unpack)]
    mode: i8,
    #[tlb(unpack)]
    exit_code: i32,
    #[tlb(unpack)]
    exit_arg: Option<i32>,
    #[tlb(unpack)]
    vm_steps: u32,
    #[tlb(unpack)]
    vm_init_state_hash: [u8; 32],
    #[tlb(unpack)]
    vm_final_state_hash: [u8; 32],
}

/// ```tlb
/// tr_phase_action$_ success:Bool valid:Bool no_funds:Bool
///   status_change:AccStatusChange
///   total_fwd_fees:(Maybe Grams) total_action_fees:(Maybe Grams)
///   result_code:int32 result_arg:(Maybe int32) tot_actions:uint16
///   spec_actions:uint16 skipped_actions:uint16 msgs_created:uint16
///   action_list_hash:bits256 tot_msg_size:StorageUsed
///   = TrActionPhase;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrActionPhase {
    success: bool,
    valid: bool,
    no_funds: bool,
    status_change: AccStatusChange,
    total_fwd_fees: Option<BigUint>,
    total_action_fees: Option<BigUint>,
    result_code: i32,
    result_arg: Option<i32>,
    tot_actions: u16,
    spec_actions: u16,
    skipped_actions: u16,
    msgs_created: u16,
    action_list_hash: [u8; 32],
    tot_msg_size: StorageUsed,
}

impl<'de> BitUnpack<'de> for TrActionPhase {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        Ok(Self {
            success: reader.unpack(())?,
            valid: reader.unpack(())?,
            no_funds: reader.unpack(())?,
            status_change: reader.unpack(())?,
            total_fwd_fees: reader.unpack_as::<_, Option<Grams>>(())?,
            total_action_fees: reader.unpack_as::<_, Option<Grams>>(())?,
            result_code: reader.unpack(())?,
            result_arg: reader.unpack(())?,
            tot_actions: reader.unpack(())?,
            spec_actions: reader.unpack(())?,
            skipped_actions: reader.unpack(())?,
            msgs_created: reader.unpack(())?,
            action_list_hash: reader.unpack(())?,
            tot_msg_size: reader.unpack(())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrBouncePhase {
    /// ```tlb
    /// tr_phase_bounce_negfunds$00 = TrBouncePhase;
    /// ```
    NegFunds,
    /// ```tlb
    /// tr_phase_bounce_nofunds$01 msg_size:StorageUsed
    ///   req_fwd_fees:Grams = TrBouncePhase;
    /// ```
    NoFunds {
        msg_size: StorageUsed,
        req_fwd_fees: BigUint,
    },
    /// ```tlb
    /// tr_phase_bounce_ok$1 msg_size:StorageUsed
    ///   msg_fees:Grams fwd_fees:Grams = TrBouncePhase;
    /// ```
    Ok {
        msg_size: StorageUsed,
        msg_fees: BigUint,
        fwd_fees: BigUint,
    },
}

impl<'de> BitUnpack<'de> for TrBouncePhase {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        let tag: bool = reader.unpack(())?;
        match tag {
            false => {
                let tag: bool = reader.unpack(())?;
                match tag {
                    false => Ok(Self::NegFunds),
                    true => Ok(Self::NoFunds {
                        msg_size: reader.unpack(())?,
                        req_fwd_fees: reader.unpack_as::<_, Grams>(())?,
                    }),
                }
            }
            true => Ok(Self::Ok {
                msg_size: reader.unpack(())?,
                msg_fees: reader.unpack_as::<_, Grams>(())?,
                fwd_fees: reader.unpack_as::<_, Grams>(())?,
            }),
        }
    }
}

/// ```tlb
/// split_merge_info$_ cur_shard_pfx_len:(## 6)
///   acc_split_depth:(## 6) this_addr:bits256 sibling_addr:bits256
///   = SplitMergeInfo;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitMergeInfo {
    cur_shard_pfx_len: u8,
    acc_split_depth: u8,
    this_addr: [u8; 32],
    sibling_addr: [u8; 32],
}

impl<'de> BitUnpack<'de> for SplitMergeInfo {
    type Args = ();

    fn unpack<R>(reader: &mut R, _args: Self::Args) -> Result<Self, R::Error>
    where
        R: BitReader<'de> + ?Sized,
    {
        Ok(Self {
            cur_shard_pfx_len: reader.unpack_as::<_, NBits<6>>(())?,
            acc_split_depth: reader.unpack_as::<_, NBits<6>>(())?,
            this_addr: reader.unpack(())?,
            sibling_addr: reader.unpack(())?,
        })
    }
}
