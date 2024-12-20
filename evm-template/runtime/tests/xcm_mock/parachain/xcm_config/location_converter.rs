use xcm_builder::{AccountKey20Aliases, DescribeAllTerminal, DescribeFamily, HashedDescription};

use crate::xcm_mock::parachain::{constants::RelayNetwork, AccountId};

type LocationToAccountId = (
    HashedDescription<AccountId, DescribeFamily<DescribeAllTerminal>>,
    AccountKey20Aliases<RelayNetwork, AccountId>,
);

pub type LocationConverter = LocationToAccountId;
