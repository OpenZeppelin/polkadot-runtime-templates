//! Substrate Parachain Node Template CLI

#![warn(missing_docs)]

mod chain_spec;
#[macro_use]
mod service;
mod cli;
mod command;
mod contracts;
mod eth;

#[cfg_attr(feature = "tanssi", path = "tanssi_rpc/mod.rs")]
mod rpc;

fn main() -> sc_cli::Result<()> {
    command::run()
}
