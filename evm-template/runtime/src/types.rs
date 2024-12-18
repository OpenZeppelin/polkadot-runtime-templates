use fp_account::EthereumSignature;
use frame_support::{
    parameter_types,
    traits::{EitherOfDiverse, InstanceFilter},
    weights::Weight,
    PalletId,
};
use frame_system::EnsureRoot;
use pallet_xcm::{EnsureXcm, IsVoiceOfBody};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use polkadot_runtime_common::impls::{
    LocatableAssetConverter, VersionedLocatableAsset, VersionedLocationConverter,
};
use scale_info::TypeInfo;
use sp_core::ConstU32;
use sp_runtime::{
    generic,
    traits::{BlakeTwo256, IdentifyAccount, Verify},
    Perbill, RuntimeDebug,
};
use sp_version::RuntimeVersion;
use xcm::{
    latest::{InteriorLocation, Junction::PalletInstance},
    VersionedLocation,
};
use xcm_builder::PayOverXcm;

use crate::{
    configs::{
        xcm_config::{self, RelayLocation},
        FeeAssetId, StakingAdminBodyId, ToSiblingBaseDeliveryFee, TransactionByteFee,
    },
    constants::HOURS,
};
pub use crate::{
    constants::{
        BLOCK_PROCESSING_VELOCITY, RELAY_CHAIN_SLOT_DURATION_MILLIS, UNINCLUDED_SEGMENT_CAPACITY,
        VERSION,
    },
    AllPalletsWithSystem, OpenZeppelinPrecompiles, Runtime, RuntimeBlockWeights, RuntimeCall,
    Treasury, XcmpQueue,
};

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    fp_self_contained::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;

/// Ethereum Signature
pub type Signature = EthereumSignature;

/// AccountId20 because 20 bytes long like H160 Ethereum Addresses
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Balance of an account.
pub type Balance = u128;

/// Identifier of an asset
pub type AssetId = u128;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// An index to a block.
pub type BlockNumber = u32;

/// The address format for describing accounts.
pub type Address = AccountId;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
    frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
    cumulus_primitives_storage_weight_reclaim::StorageWeightReclaim<Runtime>,
);

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
>;

/// Price For Sibling Parachain Delivery
pub type PriceForSiblingParachainDelivery = polkadot_runtime_common::xcm_sender::ExponentialPrice<
    FeeAssetId,
    ToSiblingBaseDeliveryFee,
    TransactionByteFee,
    XcmpQueue,
>;

/// Configures the number of blocks that can be created without submission of validity proof to the relay chain
pub type ConsensusHook = cumulus_pallet_aura_ext::FixedVelocityConsensusHook<
    Runtime,
    RELAY_CHAIN_SLOT_DURATION_MILLIS,
    BLOCK_PROCESSING_VELOCITY,
    UNINCLUDED_SEGMENT_CAPACITY,
>;

/// We allow root and the StakingAdmin to execute privileged collator selection
/// operations.
pub type CollatorSelectionUpdateOrigin = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureXcm<IsVoiceOfBody<RelayLocation, StakingAdminBodyId>>,
>;

/// These aliases are describing the Beneficiary and AssetKind for the Treasury pallet
pub type Beneficiary = VersionedLocation;
pub type AssetKind = VersionedLocatableAsset;

/// This is a type that describes how we should transfer bounties from treasury pallet
pub type TreasuryPaymaster = PayOverXcm<
    TreasuryInteriorLocation,
    xcm_config::XcmRouter,
    crate::PolkadotXcm,
    ConstU32<{ 6 * HOURS }>,
    Beneficiary,
    AssetKind,
    LocatableAssetConverter,
    VersionedLocationConverter,
>;

/// The type used to represent the kinds of proxying allowed.
/// If you are adding new pallets, consider adding new ProxyType variant
#[derive(
    Copy,
    Clone,
    Decode,
    Default,
    Encode,
    Eq,
    MaxEncodedLen,
    Ord,
    PartialEq,
    PartialOrd,
    RuntimeDebug,
    TypeInfo,
)]
pub enum ProxyType {
    /// Allows to proxy all calls
    #[default]
    Any,
    /// Allows all non-transfer calls
    NonTransfer,
    /// Allows to finish the proxy
    CancelProxy,
    /// Allows to operate with collators list (invulnerables, candidates, etc.)
    Collator,
}

impl InstanceFilter<RuntimeCall> for ProxyType {
    fn filter(&self, c: &RuntimeCall) -> bool {
        match self {
            ProxyType::Any => true,
            ProxyType::NonTransfer => !matches!(c, RuntimeCall::Balances { .. }),
            ProxyType::CancelProxy => matches!(
                c,
                RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement { .. })
                    | RuntimeCall::Multisig { .. }
            ),
            ProxyType::Collator => {
                matches!(c, RuntimeCall::CollatorSelection { .. } | RuntimeCall::Multisig { .. })
            }
        }
    }
}

// Getter types used in OpenZeppelinRuntime configuration
parameter_types! {
    pub const Version: RuntimeVersion = VERSION;
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
    pub TreasuryAccount: AccountId = Treasury::account_id();
    // The asset's interior location for the paying account. This is the Treasury
    // pallet instance (which sits at index 13).
    pub TreasuryInteriorLocation: InteriorLocation = PalletInstance(13).into();
    pub MessageQueueServiceWeight: Weight = Perbill::from_percent(35) * RuntimeBlockWeights::get().max_block;
    pub PrecompilesValue: OpenZeppelinPrecompiles<Runtime> = OpenZeppelinPrecompiles::<_>::new();
}
