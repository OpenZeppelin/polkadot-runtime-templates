use xcm_builder::{FungibleAdapter, IsConcrete};

use crate::xcm_mock::relay_chain::{
    constants::TokenLocation, location_converter::LocationConverter, AccountId, Balances,
};

pub type AssetTransactor =
    FungibleAdapter<Balances, IsConcrete<TokenLocation>, LocationConverter, AccountId, ()>;
