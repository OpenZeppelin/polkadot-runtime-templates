use xcm_builder::{
    AsPrefixedGeneralIndex, ConvertedConcreteId, FungibleAdapter, IsConcrete, NoChecking,
    NonFungiblesAdapter,
};
use xcm_executor::traits::JustTry;

use crate::xcm_mock::relay_chain::{
    constants::TokenLocation, location_converter::LocationConverter, AccountId, Balances,
};

pub type AssetTransactor =
    FungibleAdapter<Balances, IsConcrete<TokenLocation>, LocationConverter, AccountId, ()>;
