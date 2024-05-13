// Storage indices integration checks
use frame_support::traits::PalletInfo;
use parachain_template_runtime::{
    Aura, AuraExt, Authorship, Balances, CollatorSelection, CumulusXcm, MessageQueue, Multisig,
    ParachainInfo, ParachainSystem, PolkadotXcm, Proxy, Runtime, Session, Sudo, System, Timestamp,
    TransactionPayment, XcmpQueue,
};

fn assert_pallet_prefix<P: 'static>(name: &str) {
    assert_eq!(<Runtime as frame_system::Config>::PalletInfo::name::<P>(), Some(name));
}

#[test]
fn verify_pallet_prefixes() {
    assert_pallet_prefix::<System>("System");
    assert_pallet_prefix::<ParachainSystem>("ParachainSystem");
    assert_pallet_prefix::<Timestamp>("Timestamp");
    assert_pallet_prefix::<ParachainInfo>("ParachainInfo");
    assert_pallet_prefix::<Proxy>("Proxy");
    assert_pallet_prefix::<Balances>("Balances");
    assert_pallet_prefix::<TransactionPayment>("TransactionPayment");
    assert_pallet_prefix::<Sudo>("Sudo");
    assert_pallet_prefix::<Multisig>("Multisig");
    assert_pallet_prefix::<Authorship>("Authorship");
    assert_pallet_prefix::<CollatorSelection>("CollatorSelection");
    assert_pallet_prefix::<Session>("Session");
    assert_pallet_prefix::<Aura>("Aura");
    assert_pallet_prefix::<AuraExt>("AuraExt");
    assert_pallet_prefix::<XcmpQueue>("XcmpQueue");
    assert_pallet_prefix::<PolkadotXcm>("PolkadotXcm");
    assert_pallet_prefix::<CumulusXcm>("CumulusXcm");
    assert_pallet_prefix::<MessageQueue>("MessageQueue");
}
