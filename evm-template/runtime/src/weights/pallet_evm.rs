
//! Autogenerated weights for `pallet_evm`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 32.0.0
//! DATE: 2024-06-26, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `ip-172-31-15-118`, CPU: `Intel(R) Xeon(R) Platinum 8375C CPU @ 2.90GHz`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("dev")`, DB CACHE: 1024

// Executed Command:
// target/release/parachain-template-node
// benchmark
// pallet
// --steps=50
// --repeat=20
// --extrinsic=*
// --wasm-execution=compiled
// --heap-pages=4096
// --json-file=benchmarking/results/results-pallet_evm.json
// --pallet=pallet_evm
// --chain=dev
// --output=benchmarking/new-benchmarks/pallet_evm.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_evm`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_evm::WeightInfo for WeightInfo<T> {
	/// NB: For some reason, Frontier have benchmarks for create_runner, but do not have it in their WeightInfo trait.
	/// I leave it there for now, feel free to remove it in future.

	/// Storage: `BaseFee::BaseFeePerGas` (r:1 w:0)
	/// Proof: `BaseFee::BaseFeePerGas` (`max_values`: Some(1), `max_size`: Some(32), added: 527, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(116), added: 2591, mode: `MaxEncodedLen`)
	/// Storage: `EVMChainId::ChainId` (r:1 w:0)
	/// Proof: `EVMChainId::ChainId` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `EVM::AccountCodes` (r:2 w:0)
	/// Proof: `EVM::AccountCodes` (`max_values`: None, `max_size`: None, mode: `Measured`)
	/// Storage: `System::Digest` (r:1 w:0)
	/// Proof: `System::Digest` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	/// Storage: `EVM::AccountStorages` (r:1 w:0)
	/// Proof: `EVM::AccountStorages` (`max_values`: None, `max_size`: None, mode: `Measured`)
	/// The range of component `x` is `[1, 10000000]`.
	// fn runner_execute(_x: u32, ) -> Weight {
	// 	// Proof Size summary in bytes:
	// 	//  Measured:  `1083`
	// 	//  Estimated: `7023`
	// 	// Minimum execution time: 21_163_408_000 picoseconds.
	// 	Weight::from_parts(21_320_460_258, 0)
	// 		.saturating_add(Weight::from_parts(0, 7023))
	// 		.saturating_add(T::DbWeight::get().reads(7))
	// 		.saturating_add(T::DbWeight::get().writes(1))
	// }
	fn withdraw() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 3_071_000 picoseconds.
		Weight::from_parts(3_251_000, 0)
			.saturating_add(Weight::from_parts(0, 0))
	}
}
