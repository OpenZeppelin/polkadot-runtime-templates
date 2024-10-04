//! OpenGov governance config

pub mod origins;
pub use origins::{Spender, WhitelistedCaller};
pub mod tracks;

use crate::{
    constants::currency::{CENTS, GRAND},
    types::{Balance, BlockNumber},
    RuntimeOrigin,
};
