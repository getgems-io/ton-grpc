/// ```tlb
/// trans_ord$0000 credit_first:Bool
///   storage_ph:(Maybe TrStoragePhase)
///   credit_ph:(Maybe TrCreditPhase)
///   compute_ph:TrComputePhase action:(Maybe ^TrActionPhase)
///   aborted:Bool bounce:(Maybe TrBouncePhase)
///   destroyed:Bool
///   = TransactionDescr;
///
/// trans_storage$0001 storage_ph:TrStoragePhase
///   = TransactionDescr;
///
/// trans_tick_tock$001 is_tock:Bool storage_ph:TrStoragePhase
///   compute_ph:TrComputePhase action:(Maybe ^TrActionPhase)
///   aborted:Bool destroyed:Bool = TransactionDescr;
/// ///
/// split_merge_info$_ cur_shard_pfx_len:(## 6)
///   acc_split_depth:(## 6) this_addr:bits256 sibling_addr:bits256
///   = SplitMergeInfo;
/// trans_split_prepare$0100 split_info:SplitMergeInfo
///   storage_ph:(Maybe TrStoragePhase)
///   compute_ph:TrComputePhase action:(Maybe ^TrActionPhase)
///   aborted:Bool destroyed:Bool
///   = TransactionDescr;
/// trans_split_install$0101 split_info:SplitMergeInfo
///   prepare_transaction:^Transaction
///   installed:Bool = TransactionDescr;
///
/// trans_merge_prepare$0110 split_info:SplitMergeInfo
///   storage_ph:TrStoragePhase aborted:Bool
///   = TransactionDescr;
/// trans_merge_install$0111 split_info:SplitMergeInfo
///   prepare_transaction:^Transaction
///   storage_ph:(Maybe TrStoragePhase)
///   credit_ph:(Maybe TrCreditPhase)
///   compute_ph:TrComputePhase action:(Maybe ^TrActionPhase)
///   aborted:Bool destroyed:Bool
///   = TransactionDescr;
/// ```tlb
pub struct TransactionDescr {

}
