use xcm::latest::prelude::*;
use xcm_builder::{
    ConvertedConcreteId, FungibleAdapter, IsConcrete, NoChecking, NonFungiblesAdapter,
};
use xcm_executor::traits::JustTry;

use crate::parachain::{
    constants::KsmLocation, location_converter::LocationConverter, AccountId, Balances,
};

pub type AssetTransactor =
    FungibleAdapter<Balances, IsConcrete<KsmLocation>, LocationConverter, AccountId, ()>;
