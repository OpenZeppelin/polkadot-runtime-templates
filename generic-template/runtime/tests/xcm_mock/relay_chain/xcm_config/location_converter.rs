use xcm_builder::{AccountId32Aliases, DescribeAllTerminal, DescribeFamily, HashedDescription};

use crate::xcm_mock::relay_chain::{constants::RelayNetwork, AccountId};

type LocationToAccountId = (
    HashedDescription<AccountId, DescribeFamily<DescribeAllTerminal>>,
    AccountId32Aliases<RelayNetwork, AccountId>,
);

pub type LocationConverter = LocationToAccountId;
