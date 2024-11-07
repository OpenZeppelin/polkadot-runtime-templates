pub mod parachain;
pub mod relay_chain;

use sp_runtime::BuildStorage;
use sp_tracing;
use xcm::prelude::*;
use xcm_executor::traits::ConvertLocation;
use xcm_simulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain, TestExt};

pub const PARA_ALICE: [u8; 20] = [1u8; 20];
pub const ALICE: sp_runtime::AccountId32 = sp_runtime::AccountId32::new([1u8; 32]);
pub const INITIAL_BALANCE: u128 = 1_000_000_000;

decl_test_parachain! {
    pub struct ParaA {
        Runtime = parachain::Runtime,
        XcmpMessageHandler = parachain::MsgQueue,
        DmpMessageHandler = parachain::MsgQueue,
        new_ext = para_ext(1),
    }
}

decl_test_parachain! {
    pub struct ParaB {
        Runtime = parachain::Runtime,
        XcmpMessageHandler = parachain::MsgQueue,
        DmpMessageHandler = parachain::MsgQueue,
        new_ext = para_ext(2),
    }
}

decl_test_relay_chain! {
    pub struct Relay {
        Runtime = relay_chain::Runtime,
        RuntimeCall = relay_chain::RuntimeCall,
        RuntimeEvent = relay_chain::RuntimeEvent,
        XcmConfig = relay_chain::XcmConfig,
        MessageQueue = relay_chain::MessageQueue,
        System = relay_chain::System,
        new_ext = relay_ext(),
    }
}

decl_test_network! {
    pub struct MockNet {
        relay_chain = Relay,
        parachains = vec![
            (1, ParaA),
            (2, ParaB),
        ],
    }
}

pub fn parent_account_id() -> parachain::AccountId {
    let location = (Parent,);
    parachain::location_converter::LocationConverter::convert_location(&location.into()).unwrap()
}

pub fn child_account_id(para: u32) -> relay_chain::AccountId {
    let location = (Parachain(para),);
    relay_chain::location_converter::LocationConverter::convert_location(&location.into()).unwrap()
}

pub fn child_account_account_id(para: u32, who: sp_runtime::AccountId32) -> relay_chain::AccountId {
    let location = (Parachain(para), AccountId32 { network: None, id: who.into() });
    relay_chain::location_converter::LocationConverter::convert_location(&location.into()).unwrap()
}

pub fn sibling_account_account_id(para: u32, who: sp_runtime::AccountId32) -> parachain::AccountId {
    let location = (Parent, Parachain(para), AccountId32 { network: None, id: who.into() });
    parachain::location_converter::LocationConverter::convert_location(&location.into()).unwrap()
}

pub fn parent_account_account_id(who: sp_runtime::AccountId32) -> parachain::AccountId {
    let location = (Parent, AccountId32 { network: None, id: who.into() });
    parachain::location_converter::LocationConverter::convert_location(&location.into()).unwrap()
}

pub fn para_ext(para_id: u32) -> sp_io::TestExternalities {
    use parachain::{MsgQueue, Runtime, System};

    let mut t = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![
            (PARA_ALICE.into(), INITIAL_BALANCE),
            (parent_account_id(), INITIAL_BALANCE),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        sp_tracing::try_init_simple();
        System::set_block_number(1);
        MsgQueue::set_para_id(para_id.into());
    });
    ext
}

pub fn relay_ext() -> sp_io::TestExternalities {
    use relay_chain::{Runtime, System};

    let mut t = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![
            (ALICE, INITIAL_BALANCE),
            (child_account_id(1), INITIAL_BALANCE),
            (child_account_id(2), INITIAL_BALANCE),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}

pub type RelayChainPalletXcm = pallet_xcm::Pallet<relay_chain::Runtime>;
pub type ParachainPalletXcm = pallet_xcm::Pallet<parachain::Runtime>;
