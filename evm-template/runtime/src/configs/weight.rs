#[cfg(not(feature = "tanssi"))]
use openzeppelin_pallet_abstractions::ConsensusWeight;
#[cfg(feature = "tanssi")]
use openzeppelin_pallet_abstractions::TanssiWeight;
use openzeppelin_pallet_abstractions::{
    AssetsWeight, EvmWeight, GovernanceWeight, SystemWeight, XcmWeight,
};

use crate::{
    configs::OpenZeppelinRuntime,
    weights::{self, RocksDbWeight},
    Runtime,
};

impl SystemWeight for OpenZeppelinRuntime {
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
impl ConsensusWeight for OpenZeppelinRuntime {
    type CollatorSelection = weights::pallet_collator_selection::WeightInfo<Runtime>;
    type Session = weights::pallet_session::WeightInfo<Runtime>;
}

impl AssetsWeight for OpenZeppelinRuntime {
    type AssetManager = weights::pallet_asset_manager::WeightInfo<Runtime>;
    type Assets = weights::pallet_assets::WeightInfo<Runtime>;
    // TODO: fix weight
    type OracleMembership = ();
    type OrmlOracle = (); // TODO: fix weight
}

impl GovernanceWeight for OpenZeppelinRuntime {
    type ConvictionVoting = weights::pallet_conviction_voting::WeightInfo<Runtime>;
    type Referenda = weights::pallet_referenda::WeightInfo<Runtime>;
    type Sudo = weights::pallet_sudo::WeightInfo<Runtime>;
    type Treasury = weights::pallet_treasury::WeightInfo<Runtime>;
    type Whitelist = weights::pallet_whitelist::WeightInfo<Runtime>;
}

impl XcmWeight for OpenZeppelinRuntime {
    type MessageQueue = weights::pallet_message_queue::WeightInfo<Runtime>;
    type Xcm = weights::pallet_xcm::WeightInfo<Runtime>;
    type XcmTransactor = weights::pallet_xcm_transactor::WeightInfo<Runtime>;
    type XcmWeightTrader = weights::pallet_xcm_weight_trader::WeightInfo<Runtime>;
    type XcmpQueue = weights::cumulus_pallet_xcmp_queue::WeightInfo<Runtime>;
}

impl EvmWeight for OpenZeppelinRuntime {
    type Evm = weights::pallet_evm::WeightInfo<Runtime>;
}

#[cfg(feature = "tanssi")]
impl TanssiWeight for OpenZeppelinRuntime {
    type AuthorInherent = pallet_author_inherent::weights::SubstrateWeight<Runtime>;
    type AuthoritiesNoting = pallet_cc_authorities_noting::weights::SubstrateWeight<Runtime>;
}
