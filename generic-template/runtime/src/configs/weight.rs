#[cfg(not(feature = "tanssi"))]
use openzeppelin_pallet_abstractions::ConsensusWeightFull;
#[cfg(feature = "tanssi")]
use openzeppelin_pallet_abstractions::TanssiWeightFull;
use openzeppelin_pallet_abstractions::{AssetsWeightFull, SystemWeightFull, XcmWeightFull, GovernanceWeightFull};

use crate::{
    configs::OpenZeppelinRuntime,
    weights::{self, RocksDbWeight},
    Runtime,
};

impl SystemWeightFull for OpenZeppelinRuntime {
    type Balances = weights::pallet_balances::WeightInfo<Runtime>;
    type DbWeight = RocksDbWeight;
    type Multisig = weights::pallet_multisig::WeightInfo<Runtime>;
    type ParachainSystem = weights::cumulus_pallet_parachain_system::WeightInfo<Runtime>;
    type Preimage = weights::pallet_preimage::WeightInfo<Runtime>;
    type Proxy = weights::pallet_proxy::WeightInfo<Runtime>;
    type Scheduler = weights::pallet_scheduler::WeightInfo<Runtime>;
    type Timestamp = weights::pallet_timestamp::WeightInfo<Runtime>;
    type Utility = weights::pallet_utility::WeightInfo<Runtime>;
}

#[cfg(not(feature = "tanssi"))]
impl ConsensusWeightFull for OpenZeppelinRuntime {
    type CollatorSelection = weights::pallet_collator_selection::WeightInfo<Runtime>;
    type Session = weights::pallet_session::WeightInfo<Runtime>;
}

impl AssetsWeightFull for OpenZeppelinRuntime {
    type AssetManager = weights::pallet_asset_manager::WeightInfo<Runtime>;
    //TODO: fix weight
    type AssetTxPayment = ();
    type Assets = weights::pallet_assets::WeightInfo<Runtime>;
    // TODO: fix weight on release
    type OracleMembership = ();
    type OrmlOracle = ();
    // TODO: fix weight
    type TransactionPayment = weights::pallet_transaction_payment::WeightInfo<Runtime>;
}

impl GovernanceWeightFull for OpenZeppelinRuntime {
    type ConvictionVoting = weights::pallet_conviction_voting::WeightInfo<Runtime>;
    type Referenda = weights::pallet_referenda::WeightInfo<Runtime>;
    type Sudo = weights::pallet_sudo::WeightInfo<Runtime>;
    type Treasury = weights::pallet_treasury::WeightInfo<Runtime>;
    type Whitelist = weights::pallet_whitelist::WeightInfo<Runtime>;
}

impl XcmWeightFull for OpenZeppelinRuntime {
    type MessageQueue = weights::pallet_message_queue::WeightInfo<Runtime>;
    type Xcm = weights::pallet_xcm::WeightInfo<Runtime>;
    type XcmTransactor = weights::pallet_xcm_transactor::WeightInfo<Runtime>;
    type XcmWeightTrader = weights::pallet_xcm_weight_trader::WeightInfo<Runtime>;
    type XcmpQueue = weights::cumulus_pallet_xcmp_queue::WeightInfo<Runtime>;
}

#[cfg(feature = "tanssi")]
impl TanssiWeightFull for OpenZeppelinRuntime {
    type AuthorInherent = pallet_author_inherent::weights::SubstrateWeight<Runtime>;
    type AuthoritiesNoting = pallet_cc_authorities_noting::weights::SubstrateWeight<Runtime>;
}
