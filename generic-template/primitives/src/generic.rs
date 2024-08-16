//! Generic assigned types
use sp_runtime::{
    generic,
    traits::{BlakeTwo256, IdentifyAccount, Verify},
    MultiAddress, MultiSignature,
};

use super::*;

/// Alias to 512-bit hash when used in the context of a transaction signature on
/// the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it
/// equivalent to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic<RuntimeCall> =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, ()>;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block<RuntimeCall> = generic::Block<Header, UncheckedExtrinsic<RuntimeCall>>;

// The SignedExtension to the basic transaction logic.
// pub type SignedExtra<Runtime> = (
//     frame_system::CheckNonZeroSender<Runtime>,
//     frame_system::CheckSpecVersion<Runtime>,
//     frame_system::CheckTxVersion<Runtime>,
//     frame_system::CheckGenesis<Runtime>,
//     frame_system::CheckEra<Runtime>,
//     frame_system::CheckNonce<Runtime>,
//     frame_system::CheckWeight<Runtime>,
//     pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
//     cumulus_primitives_storage_weight_reclaim::StorageWeightReclaim<Runtime>,
// );
