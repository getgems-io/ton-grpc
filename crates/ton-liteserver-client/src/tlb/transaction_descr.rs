use crate::tlb::currency_collection::CurrencyCollection;
use crate::tlb::storage_used::StorageUsed;
use crate::tlb::transaction::Transaction;
use num_bigint::BigUint;
use toner::tlb::bits::{NBits, VarInt};
use toner::tlb::{Data, ParseFully, Ref};
use toner::ton::currency::Grams;
use toner_tlb_macros::{BitUnpack, CellDeserialize};

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
#[derive(Debug, Clone, PartialEq, Eq, BitUnpack)]
pub struct TrStoragePhase {
    #[tlb(unpack_as = "Grams")]
    storage_fees_collected: BigUint,
    #[tlb(unpack_as = "Option<Grams>")]
    storage_fees_due: Option<BigUint>,
    status_change: AccStatusChange,
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
#[derive(Debug, Copy, Clone, PartialEq, Eq, BitUnpack)]
pub enum AccStatusChange {
    #[tlb(tag = "0b0")]
    Unchanged,
    #[tlb(tag = "0b10")]
    Frozen,
    #[tlb(tag = "0b11")]
    Deleted,
}

/// ```tlb
/// cskip_no_state$00 = ComputeSkipReason;
/// cskip_bad_state$01 = ComputeSkipReason;
/// cskip_no_gas$10 = ComputeSkipReason;
/// cskip_suspended$110 = ComputeSkipReason;
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, BitUnpack)]
pub enum ComputeSkipReason {
    #[tlb(tag = "0b00")]
    NoState,
    #[tlb(tag = "0b01")]
    BadState,
    #[tlb(tag = "0b10")]
    NoGas,
    #[tlb(tag = "0b110")]
    Suspended,
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
#[derive(Debug, Clone, PartialEq, Eq, CellDeserialize)]
pub enum TrComputePhase {
    #[tlb(tag = "$0")]
    Skipped {
        #[tlb(unpack)]
        reason: ComputeSkipReason,
    },
    #[tlb(tag = "$1")]
    Vm {
        #[tlb(unpack)]
        success: bool,
        #[tlb(unpack)]
        msg_state_used: bool,
        #[tlb(unpack)]
        account_activated: bool,
        #[tlb(unpack_as = "Grams")]
        gas_fees: BigUint,
        #[tlb(separate_cell_start, unpack_as = "VarInt<3>")]
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
        #[tlb(separate_cell_end, unpack)]
        vm_final_state_hash: [u8; 32],
    },
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
#[derive(Debug, Clone, PartialEq, Eq, BitUnpack)]
pub struct TrActionPhase {
    success: bool,
    valid: bool,
    no_funds: bool,
    status_change: AccStatusChange,
    #[tlb(unpack_as = "Option<Grams>")]
    total_fwd_fees: Option<BigUint>,
    #[tlb(unpack_as = "Option<Grams>")]
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

/// ```tlb
/// tr_phase_bounce_negfunds$00 = TrBouncePhase;
/// tr_phase_bounce_nofunds$01 msg_size:StorageUsed
///   req_fwd_fees:Grams = TrBouncePhase;
/// tr_phase_bounce_ok$1 msg_size:StorageUsed
///   msg_fees:Grams fwd_fees:Grams = TrBouncePhase;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, BitUnpack)]
pub enum TrBouncePhase {
    #[tlb(tag = "$00")]
    NegFunds,
    #[tlb(tag = "$01")]
    NoFunds {
        msg_size: StorageUsed,
        #[tlb(unpack_as = "Grams")]
        req_fwd_fees: BigUint,
    },
    #[tlb(tag = "$1")]
    Ok {
        msg_size: StorageUsed,
        #[tlb(unpack_as = "Grams")]
        msg_fees: BigUint,
        #[tlb(unpack_as = "Grams")]
        fwd_fees: BigUint,
    },
}

/// ```tlb
/// split_merge_info$_ cur_shard_pfx_len:(## 6)
///   acc_split_depth:(## 6) this_addr:bits256 sibling_addr:bits256
///   = SplitMergeInfo;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, BitUnpack)]
pub struct SplitMergeInfo {
    #[tlb(unpack_as = "NBits<6>")]
    cur_shard_pfx_len: u8,
    #[tlb(unpack_as = "NBits<6>")]
    acc_split_depth: u8,
    this_addr: [u8; 32],
    sibling_addr: [u8; 32],
}
