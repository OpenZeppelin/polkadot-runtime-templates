use openzeppelin_polkadot_wrappers_proc::openzeppelin_runtime_apis;

#[cfg(feature = "runtime-benchmarks")]
use crate::constants::currency::{CENTS, EXISTENTIAL_DEPOSIT};
#[cfg(not(feature = "async-backing"))]
use crate::Aura;
#[cfg(feature = "async-backing")]
use crate::{configs::XcmExecutorConfig, constants::SLOT_DURATION, types::ConsensusHook};
use crate::{
    constants::VERSION,
    types::{AccountId, Balance, Block, Executive, Nonce},
    InherentDataExt, ParachainSystem, Runtime, RuntimeCall, RuntimeGenesisConfig, SessionKeys,
    System, TransactionPayment,
};

#[cfg(feature = "runtime-benchmarks")]
type ExistentialDeposit = sp_core::ConstU128<EXISTENTIAL_DEPOSIT>;

#[openzeppelin_runtime_apis]
mod apis {
    type Runtime = Runtime;
    type Block = Block;

    #[abstraction]
    mod assets {
        type RuntimeCall = RuntimeCall;
        type TransactionPayment = TransactionPayment;
        type Balance = Balance;
    }

    #[abstraction]
    mod consensus {
        type SessionKeys = SessionKeys;
        #[cfg(not(feature = "async-backing"))]
        type Aura = Aura;
        #[cfg(feature = "async-backing")]
        type SlotDuration = SLOT_DURATION;
        #[cfg(feature = "async-backing")]
        type ConsensusHook = ConsensusHook;
    }

    #[abstraction]
    mod system {
        type Executive = Executive;
        type System = System;
        type ParachainSystem = ParachainSystem;
        type RuntimeVersion = VERSION;
        type AccountId = AccountId;
        type Nonce = Nonce;
        type RuntimeGenesisConfig = RuntimeGenesisConfig;
        type RuntimeBlockWeights = RuntimeBlockWeights;
    }

    #[abstraction]
    mod benchmarks {
        type AllPalletsWithSystem = AllPalletsWithSystem;
        type Assets = Assets;
        type AssetManager = AssetManager;
        type AssetType = AssetType;
        type RuntimeOrigin = RuntimeOrigin;
        type RelayLocation = RelayLocation;
        type ParachainSystem = ParachainSystem;
        type System = System;
        type ExistentialDeposit = ExistentialDeposit;
        type AssetId = AssetId;
        type XCMConfig = XcmExecutorConfig;
        type AccountId = AccountId;
        type Cents = CENTS;
        type FeeAssetId = FeeAssetId;
        type TransactionByteFee = TransactionByteFee;
        type Address = Address;
        type Balances = Balances;
    }
}
