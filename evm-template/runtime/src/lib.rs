#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub mod configs;
pub mod constants;
mod precompiles;
pub use precompiles::OpenZeppelinPrecompiles;
mod types;
mod weights;

use frame_support::{
    genesis_builder_helper::{build_state, get_preset},
    traits::OnFinalize,
    weights::{Weight, WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial},
};
use pallet_ethereum::{
    Call::transact, Transaction as EthereumTransaction, TransactionAction, TransactionData,
    TransactionStatus,
};
use pallet_evm::{Account as EVMAccount, FeeCalculator, Runner};
use smallvec::smallvec;
use sp_api::impl_runtime_apis;
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata, H160, H256, U256};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{
    impl_opaque_keys,
    traits::{
        Block as BlockT, DispatchInfoOf, Dispatchable, Get, PostDispatchInfoOf, UniqueSaturatedInto,
    },
    transaction_validity::{TransactionSource, TransactionValidity, TransactionValidityError},
    ApplyExtrinsicResult,
};
pub use sp_runtime::{MultiAddress, Perbill, Permill};
use sp_std::prelude::{Vec, *};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use crate::{
    configs::pallet_custom_origins,
    constants::{currency::MILLICENTS, POLY_DEGREE, P_FACTOR, Q_FACTOR, VERSION},
    weights::ExtrinsicBaseWeight,
};
pub use crate::{
    configs::RuntimeBlockWeights,
    types::{
        AccountId, Balance, Block, BlockNumber, Executive, Nonce, Signature, UncheckedExtrinsic,
    },
};
#[cfg(feature = "async-backing")]
use crate::{constants::SLOT_DURATION, types::ConsensusHook};

impl fp_self_contained::SelfContainedCall for RuntimeCall {
    type SignedInfo = H160;

    fn is_self_contained(&self) -> bool {
        match self {
            RuntimeCall::Ethereum(call) => call.is_self_contained(),
            _ => false,
        }
    }

    fn check_self_contained(&self) -> Option<Result<Self::SignedInfo, TransactionValidityError>> {
        match self {
            RuntimeCall::Ethereum(call) => call.check_self_contained(),
            _ => None,
        }
    }

    fn validate_self_contained(
        &self,
        info: &Self::SignedInfo,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Option<TransactionValidity> {
        match self {
            RuntimeCall::Ethereum(call) => call.validate_self_contained(info, dispatch_info, len),
            _ => None,
        }
    }

    fn pre_dispatch_self_contained(
        &self,
        info: &Self::SignedInfo,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Option<Result<(), TransactionValidityError>> {
        match self {
            RuntimeCall::Ethereum(call) =>
                call.pre_dispatch_self_contained(info, dispatch_info, len),
            _ => None,
        }
    }

    fn apply_self_contained(
        self,
        info: Self::SignedInfo,
    ) -> Option<sp_runtime::DispatchResultWithInfo<PostDispatchInfoOf<Self>>> {
        match self {
            call @ RuntimeCall::Ethereum(pallet_ethereum::Call::transact { .. }) =>
                Some(call.dispatch(RuntimeOrigin::from(
                    pallet_ethereum::RawOrigin::EthereumTransaction(info),
                ))),
            _ => None,
        }
    }
}

/// Handles converting a weight scalar to a fee value, based on the scale and
/// granularity of the node's balance type.
///
/// This should typically create a mapping between the following ranges:
///   - `[0, MAXIMUM_BLOCK_WEIGHT]`
///   - `[Balance::min, Balance::max]`
///
/// Yet, it can be used for any other sort of change to weight-fee. Some
/// examples being:
///   - Setting it to `0` will essentially disable the weight fee.
///   - Setting it to `1` will cause the literal `#[weight = x]` values to be
///     charged.
pub struct WeightToFee;

impl WeightToFeePolynomial for WeightToFee {
    type Balance = Balance;

    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        // in Paseo, extrinsic base weight (smallest non-zero weight) is mapped to 1
        // MILLIUNIT: in our template, we map to 1/10 of that, or 1/10 MILLIUNIT
        let p = MILLICENTS / P_FACTOR;
        let q = Q_FACTOR * Balance::from(ExtrinsicBaseWeight::get().ref_time());
        smallvec![WeightToFeeCoefficient {
            degree: POLY_DEGREE,
            negative: false,
            coeff_frac: Perbill::from_rational(p % q, q),
            coeff_integer: p / q,
        }]
    }
}

/// Opaque types. These are used by the CLI to instantiate machinery that don't
/// need to know the specifics of the runtime. They can then be made to be
/// agnostic over specific formats of data like extrinsics, allowing for them to
/// continue syncing the network through upgrades to even the core data
/// structures.
pub mod opaque {
    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;
    use sp_runtime::{
        generic,
        traits::{BlakeTwo256, Hash as HashT},
    };

    use super::*;
    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;
    /// Opaque block hash type.
    pub type Hash = <BlakeTwo256 as HashT>::Output;
}

impl_opaque_keys! {
    pub struct SessionKeys {
        pub aura: Aura,
    }
}

/// The version information used to identify this runtime when compiled
/// natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    use crate::constants::VERSION;

    NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask
    )]
    pub struct Runtime;

    #[runtime::pallet_index(0)]
    pub type System = frame_system;
    #[runtime::pallet_index(1)]
    pub type ParachainSystem = cumulus_pallet_parachain_system;
    #[runtime::pallet_index(2)]
    pub type Timestamp = pallet_timestamp;
    #[runtime::pallet_index(3)]
    pub type ParachainInfo = parachain_info;
    #[runtime::pallet_index(4)]
    pub type Proxy = pallet_proxy;
    #[runtime::pallet_index(5)]
    pub type Utility = pallet_utility;
    #[runtime::pallet_index(6)]
    pub type Multisig = pallet_multisig;
    #[runtime::pallet_index(7)]
    pub type Scheduler = pallet_scheduler;
    #[runtime::pallet_index(8)]
    pub type Preimage = pallet_preimage;

    // Monetary stuff.
    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type TransactionPayment = pallet_transaction_payment;
    #[runtime::pallet_index(12)]
    pub type Assets = pallet_assets;
    #[runtime::pallet_index(13)]
    pub type Treasury = pallet_treasury;
    #[runtime::pallet_index(14)]
    pub type AssetManager = pallet_asset_manager;

    // Governance
    #[runtime::pallet_index(15)]
    pub type Sudo = pallet_sudo;
    #[runtime::pallet_index(16)]
    pub type ConvictionVoting = pallet_conviction_voting;
    #[runtime::pallet_index(17)]
    pub type Referenda = pallet_referenda;
    #[runtime::pallet_index(18)]
    pub type Origins = pallet_custom_origins;
    #[runtime::pallet_index(19)]
    pub type Whitelist = pallet_whitelist;

    // Collator support. The order of these 4 are important and shall not change.
    #[runtime::pallet_index(20)]
    pub type Authorship = pallet_authorship;
    #[runtime::pallet_index(21)]
    pub type CollatorSelection = pallet_collator_selection;
    #[runtime::pallet_index(22)]
    pub type Session = pallet_session;
    #[runtime::pallet_index(23)]
    pub type Aura = pallet_aura;
    #[runtime::pallet_index(24)]
    pub type AuraExt = cumulus_pallet_aura_ext;

    // XCM helpers.
    #[runtime::pallet_index(30)]
    pub type XcmpQueue = cumulus_pallet_xcmp_queue;
    #[runtime::pallet_index(31)]
    pub type PolkadotXcm = pallet_xcm;
    #[runtime::pallet_index(32)]
    pub type CumulusXcm = cumulus_pallet_xcm;
    #[runtime::pallet_index(33)]
    pub type MessageQueue = pallet_message_queue;

    // EVM
    #[runtime::pallet_index(40)]
    pub type Ethereum = pallet_ethereum;
    #[runtime::pallet_index(41)]
    pub type EVM = pallet_evm;
    #[runtime::pallet_index(42)]
    pub type BaseFee = pallet_base_fee;
    #[runtime::pallet_index(43)]
    pub type EVMChainId = pallet_evm_chain_id;
}

cumulus_pallet_parachain_system::register_validate_block! {
    Runtime = Runtime,
    BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}

impl_runtime_apis! {
    impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
        fn slot_duration() -> sp_consensus_aura::SlotDuration {
            #[cfg(feature = "async-backing")]
            return sp_consensus_aura::SlotDuration::from_millis(SLOT_DURATION);
            #[cfg(not(feature = "async-backing"))]
            sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
        }

        fn authorities() -> Vec<AuraId> {
            pallet_aura::Authorities::<Runtime>::get().into_inner()
        }
    }

    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block)
        }

        fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> sp_std::vec::Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
        for Runtime
    {
        fn query_call_info(
            call: RuntimeCall,
            len: u32,
        ) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_call_info(call, len)
        }
        fn query_call_fee_details(
            call: RuntimeCall,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_call_fee_details(call, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl fp_rpc::EthereumRuntimeRPCApi<Block> for Runtime {
        /// Returns runtime defined pallet_evm::ChainId.
        fn chain_id() -> u64 {
            <Runtime as pallet_evm::Config>::ChainId::get()
        }

        /// Returns pallet_evm::Accounts by address.
        fn account_basic(address: H160) -> EVMAccount {
            let (account, _) = pallet_evm::Pallet::<Runtime>::account_basic(&address);
            account
        }

        /// Returns FixedGasPrice::min_gas_price
        fn gas_price() -> U256 {
            let (gas_price, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();
            gas_price
        }

        /// For a given account address, returns pallet_evm::AccountCodes.
        fn account_code_at(address: H160) -> Vec<u8> {
            pallet_evm::AccountCodes::<Runtime>::get(address)
        }

        /// Returns the converted FindAuthor::find_author authority id.
        fn author() -> H160 {
            <pallet_evm::Pallet<Runtime>>::find_author()
        }

        /// For a given account address and index, returns pallet_evm::AccountStorages.
        fn storage_at(address: H160, index: U256) -> H256 {
            let mut tmp = [0u8; 32];
            index.to_big_endian(&mut tmp);
            pallet_evm::AccountStorages::<Runtime>::get(address, H256::from_slice(&tmp[..]))
        }

        /// Returns a frame_ethereum::call response.
        fn call(
            from: H160,
            to: H160,
            data: Vec<u8>,
            value: U256,
            gas_limit: U256,
            max_fee_per_gas: Option<U256>,
            max_priority_fee_per_gas: Option<U256>,
            nonce: Option<U256>,
            estimate: bool,
            access_list: Option<Vec<(H160, Vec<H256>)>>,
        ) -> Result<pallet_evm::CallInfo, sp_runtime::DispatchError> {
            let config = if estimate {
                let mut config = <Runtime as pallet_evm::Config>::config().clone();
                config.estimate = true;
                Some(config)
            } else {
                None
            };

            let gas_limit = gas_limit.min(u64::MAX.into());
            let transaction_data = TransactionData::new(
                TransactionAction::Call(to),
                data.clone(),
                nonce.unwrap_or_default(),
                gas_limit,
                None,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                value,
                Some(<Runtime as pallet_evm::Config>::ChainId::get()),
                access_list.clone().unwrap_or_default(),
            );
            let (weight_limit, proof_size_base_cost) = pallet_ethereum::Pallet::<Runtime>::transaction_weight(&transaction_data);

            <Runtime as pallet_evm::Config>::Runner::call(
                from,
                to,
                data,
                value,
                gas_limit.unique_saturated_into(),
                max_fee_per_gas,
                max_priority_fee_per_gas,
                nonce,
                access_list.unwrap_or_default(),
                false,
                true,
                weight_limit,
                proof_size_base_cost,
                config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config()),
            ).map_err(|err| err.error.into())
        }

        /// Returns a frame_ethereum::create response.
        fn create(
            from: H160,
            data: Vec<u8>,
            value: U256,
            gas_limit: U256,
            max_fee_per_gas: Option<U256>,
            max_priority_fee_per_gas: Option<U256>,
            nonce: Option<U256>,
            estimate: bool,
            access_list: Option<Vec<(H160, Vec<H256>)>>,
        ) -> Result<pallet_evm::CreateInfo, sp_runtime::DispatchError> {
            let config = if estimate {
                let mut config = <Runtime as pallet_evm::Config>::config().clone();
                config.estimate = true;
                Some(config)
            } else {
                None
            };

            let transaction_data = TransactionData::new(
                TransactionAction::Create,
                data.clone(),
                nonce.unwrap_or_default(),
                gas_limit,
                None,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                value,
                Some(<Runtime as pallet_evm::Config>::ChainId::get()),
                access_list.clone().unwrap_or_default(),
            );
            let (weight_limit, proof_size_base_cost) = pallet_ethereum::Pallet::<Runtime>::transaction_weight(&transaction_data);

            <Runtime as pallet_evm::Config>::Runner::create(
                from,
                data,
                value,
                gas_limit.unique_saturated_into(),
                max_fee_per_gas,
                max_priority_fee_per_gas,
                nonce,
                access_list.unwrap_or_default(),
                false,
                true,
                weight_limit,
                proof_size_base_cost,
                config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config()),
            ).map_err(|err| err.error.into())
        }

        /// Return the current transaction status.
        fn current_transaction_statuses() -> Option<Vec<TransactionStatus>> {
            pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
        }

        /// Return the current block.
        fn current_block() -> Option<pallet_ethereum::Block> {
            pallet_ethereum::CurrentBlock::<Runtime>::get()
        }

        /// Return the current receipts.
        fn current_receipts() -> Option<Vec<pallet_ethereum::Receipt>> {
            pallet_ethereum::CurrentReceipts::<Runtime>::get()
        }

        /// Return all the current data for a block in a single runtime call.
        fn current_all() -> (
            Option<pallet_ethereum::Block>,
            Option<Vec<pallet_ethereum::Receipt>>,
            Option<Vec<TransactionStatus>>
        ) {
            (
                pallet_ethereum::CurrentBlock::<Runtime>::get(),
                pallet_ethereum::CurrentReceipts::<Runtime>::get(),
                pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
            )
        }

        /// Receives a `Vec<OpaqueExtrinsic>` and filters out all the non-ethereum transactions.
        fn extrinsic_filter(
            xts: Vec<<Block as BlockT>::Extrinsic>,
        ) -> Vec<EthereumTransaction> {
            xts.into_iter().filter_map(|xt| match xt.0.function {
                RuntimeCall::Ethereum(transact { transaction }) => Some(transaction),
                _ => None
            }).collect::<Vec<EthereumTransaction>>()
        }

        /// Return the elasticity multiplier.
        fn elasticity() -> Option<Permill> {
            Some(pallet_base_fee::Elasticity::<Runtime>::get())
        }

        /// Used to determine if gas limit multiplier for non-transactional calls (eth_call/estimateGas)
        /// is supported.
        fn gas_limit_multiplier_support() {}

        /// Return the pending block.
        fn pending_block(
            xts: Vec<<Block as BlockT>::Extrinsic>,
        ) -> (Option<pallet_ethereum::Block>, Option<Vec<TransactionStatus>>) {
            for ext in xts.into_iter() {
                let _ = Executive::apply_extrinsic(ext);
            }

            Ethereum::on_finalize(System::block_number() + 1);

            (
                pallet_ethereum::CurrentBlock::<Runtime>::get(),
                pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
            )
        }

        fn initialize_pending_block(header: &<Block as BlockT>::Header) {
            Executive::initialize_block(header);
        }
    }

    impl fp_rpc::ConvertTransactionRuntimeApi<Block> for Runtime {
        /// Converts an ethereum transaction into a transaction suitable for the runtime.
        fn convert_transaction(transaction: EthereumTransaction) -> <Block as BlockT>::Extrinsic {
            UncheckedExtrinsic::new_unsigned(
                pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
            )
        }
    }



    impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
        fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
            ParachainSystem::collect_collation_info(header)
        }
    }

    #[cfg(feature = "async-backing")]
    impl cumulus_primitives_aura::AuraUnincludedSegmentApi<Block> for Runtime {
        fn can_build_upon(
            included_hash: <Block as BlockT>::Hash,
            slot: cumulus_primitives_aura::Slot
        ) -> bool {
            ConsensusHook::can_build_upon(included_hash, slot)
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            use crate::configs::RuntimeBlockWeights;

            let weight = Executive::try_runtime_upgrade(checks).unwrap();
            (weight, RuntimeBlockWeights::get().max_block)
        }

        fn execute_block(
            block: Block,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect,
        ) -> Weight {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here.
            Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{Benchmarking, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;
            use frame_system_benchmarking::Pallet as SystemBench;
            use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

            use pallet_xcm::benchmarking::Pallet as PalletXcmExtrinsicsBenchmark;

            use crate::*;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);

            let storage_info = AllPalletsWithSystem::storage_info();
            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{BenchmarkError, Benchmarking, BenchmarkBatch};
            use frame_support::parameter_types;
            use cumulus_primitives_core::ParaId;
            use frame_system_benchmarking::Pallet as SystemBench;

            use crate::{*, types::*, configs::*, constants::currency::CENTS};

            #[allow(non_local_definitions)]
            impl frame_system_benchmarking::Config for Runtime {
                fn setup_set_code_requirements(code: &sp_std::vec::Vec<u8>) -> Result<(), BenchmarkError> {
                    ParachainSystem::initialize_for_set_code_benchmark(code.len() as u32);
                    Ok(())
                }

                fn verify_set_code() {
                    System::assert_last_event(cumulus_pallet_parachain_system::Event::<Runtime>::ValidationFunctionStored.into());
                }
            }

            parameter_types! {
                pub const RandomParaId: ParaId = ParaId::new(43211234);
                pub ExistentialDepositAsset: Option<Asset> = Some((
                    RelayLocation::get(),
                    ExistentialDeposit::get()
                ).into());
                /// The base fee for the message delivery fees. Kusama is based for the reference.
                pub const ToParentBaseDeliveryFee: u128 = CENTS.saturating_mul(3);
                pub const InitialTransferAssetAmount: u128 = 4001070000100;
            }
            pub type PriceForParentDelivery = polkadot_runtime_common::xcm_sender::ExponentialPrice<
                FeeAssetId,
                ToParentBaseDeliveryFee,
                TransactionByteFee,
                ParachainSystem,
            >;
            use pallet_xcm::benchmarking::Pallet as PalletXcmExtrinsicsBenchmark;
            use xcm::latest::prelude::{Asset, AssetId, Assets as AssetList, Fungible, Location, Parachain, Parent, ParentThen, PalletInstance, GeneralIndex};

            #[allow(non_local_definitions)]
            impl pallet_xcm::benchmarking::Config for Runtime {
                type DeliveryHelper = cumulus_primitives_utility::ToParentDeliveryHelper<
                    xcm_config::XcmConfig,
                    ExistentialDepositAsset,
                    PriceForParentDelivery,
                >;

                fn reachable_dest() -> Option<Location> {
                    Some(Parent.into())
                }

                fn teleportable_asset_and_dest() -> Option<(Asset, Location)> {
                    None
                }

                fn reserve_transferable_asset_and_dest() -> Option<(Asset, Location)> {
                    use frame_support::traits::PalletInfoAccess;
                    use xcm_primitives::AssetTypeGetter;
                    use frame_system::RawOrigin;

                    // set up fee asset
                    let fee_location = RelayLocation::get();
                    let who: AccountId = frame_benchmarking::whitelisted_caller();

                    let Some(location_v3) = xcm::v3::Location::try_from(fee_location.clone()).ok() else {
                        return None;
                    };
                    let asset_type = AssetType::Xcm(location_v3);

                    let local_asset_id: crate::types::AssetId = asset_type.clone().into();
                    let manager_id = AssetManager::account_id();
                    let _ = Assets::force_create(RuntimeOrigin::root(), local_asset_id.clone().into(), manager_id.clone(), true, 1);
                    let _ = Assets::mint(
                        RawOrigin::Signed(manager_id.clone()).into(),
                        local_asset_id.into(),
                        who,
                        InitialTransferAssetAmount::get(),
                    );
                    AssetManager::set_asset_type_asset_id(asset_type.clone(), local_asset_id.into());

                    // open a mock parachain channel
                    ParachainSystem::open_outbound_hrmp_channel_for_benchmarks_or_tests(
                        RandomParaId::get().into()
                    );

                    // set up transfer asset
                    let initial_asset_amount: u128 = InitialTransferAssetAmount::get();
                    let (asset_id, _, _) = pallet_assets::benchmarking::create_default_minted_asset::<
                        Runtime,
                        ()
                    >(true, initial_asset_amount);

                    let asset_id_u128: u128 = asset_id.into();
                    let self_reserve = Location {
                        parents: 0,
                        interior: [
                            PalletInstance(<Assets as PalletInfoAccess>::index() as u8), GeneralIndex(asset_id_u128)
                        ].into()
                    };

                    let Some(location_v3) = xcm::v3::Location::try_from(self_reserve.clone()).ok() else {
                        return None;
                    };
                    let asset_type = AssetType::Xcm(location_v3);
                    AssetManager::set_asset_type_asset_id(asset_type.clone(), asset_id_u128);

                    let asset = Asset {
                        fun: Fungible(ExistentialDeposit::get()),
                        id: AssetId(self_reserve.into())
                    }.into();
                    Some((
                        asset,
                        ParentThen(Parachain(RandomParaId::get().into()).into()).into(),
                    ))
                }

                fn set_up_complex_asset_transfer(
                ) -> Option<(AssetList, u32, Location, Box<dyn FnOnce()>)> {
                    use frame_support::traits::PalletInfoAccess;
                    use xcm_primitives::AssetTypeGetter;
                    // set up local asset
                    let initial_asset_amount: u128 = 1000000011;

                    let (asset_id, _, _) = pallet_assets::benchmarking::create_default_minted_asset::<
                        Runtime,
                        ()
                    >(true, initial_asset_amount);

                    let asset_id_u128: u128 = asset_id.into();

                    let self_reserve = Location {
                        parents:0,
                        interior: [
                            PalletInstance(<Assets as PalletInfoAccess>::index() as u8), GeneralIndex(asset_id_u128)
                        ].into()
                    };

                    let Some(location_v3) = xcm::v3::Location::try_from(self_reserve.clone()).ok() else {
                        return None;
                    };
                    let asset_type = AssetType::Xcm(location_v3);
                    AssetManager::set_asset_type_asset_id(asset_type.clone(), asset_id_u128);

                    let destination: xcm::v4::Location = Parent.into();

                    // set up fee asset
                    let fee_amount: u128 = <Runtime as pallet_balances::Config>::ExistentialDeposit::get();
                    let asset_amount: u128 = 10;
                    let fee_asset: Asset = (self_reserve.clone(), fee_amount).into();
                    let transfer_asset: Asset = (self_reserve.clone(), asset_amount).into();

                    let assets: cumulus_primitives_core::Assets = vec![fee_asset.clone(), transfer_asset].into();
                    let fee_index: u32 = 0;

                    let who = frame_benchmarking::whitelisted_caller();

                    let verify: Box<dyn FnOnce()> = Box::new(move || {
                        // verify balance after transfer, decreased by
                        // transferred amount (and delivery fees)
                        assert!(Assets::balance(asset_id_u128, &who) <= initial_asset_amount - fee_amount);
                    });

                    Some((assets, fee_index, destination, verify))
                }

                fn get_asset() -> Asset {
                    use xcm_primitives::AssetTypeGetter;
                    let location = Location::parent();
                    let asset_id = AssetId(location.clone());
                    let asset = Asset {
                        id: asset_id.clone(),
                        fun: Fungible(ExistentialDeposit::get()),
                    };
                    let Some(location_v3) = xcm::v3::Location::try_from(location).ok() else {
                        return asset;
                    };
                    let asset_type = AssetType::Xcm(location_v3);
                    let local_asset_id: crate::types::AssetId = asset_type.clone().into();
                    let manager_id = AssetManager::account_id();
                    let _ = Assets::force_create(RuntimeOrigin::root(), local_asset_id.clone().into(), manager_id, true, 1);
                    AssetManager::set_asset_type_asset_id(asset_type.clone(), local_asset_id);
                    asset
                }
            }

            use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

            #[allow(non_local_definitions)]
            impl cumulus_pallet_session_benchmarking::Config for Runtime {}

            use frame_support::traits::WhitelistedStorageKeys;
            let whitelist = AllPalletsWithSystem::whitelisted_storage_keys();

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);
            add_benchmarks!(params, batches);

            if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
            Ok(batches)
        }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
            build_state::<RuntimeGenesisConfig>(config)
        }

        fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            get_preset::<RuntimeGenesisConfig>(id, |_| None)
        }

        fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
            Default::default()
        }
    }
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmark;
