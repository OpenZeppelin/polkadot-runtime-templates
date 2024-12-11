use xcm_builder::{FungibleAdapter, IsConcrete};

use crate::parachain::{
    constants::KsmLocation, location_converter::LocationConverter, AccountId, Balances,
};

pub type AssetTransactor =
    FungibleAdapter<Balances, IsConcrete<KsmLocation>, LocationConverter, AccountId, ()>;
