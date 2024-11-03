
//! Autogenerated weights for `pallet_asset_manager`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 42.0.0
//! DATE: 2024-11-03, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `ip-172-31-15-118`, CPU: `Intel(R) Xeon(R) Platinum 8375C CPU @ 2.90GHz`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("dev")`, DB CACHE: 1024

// Executed Command:
// target/release/generic-template-node
// benchmark
// pallet
// --steps=50
// --repeat=20
// --extrinsic=*
// --wasm-execution=compiled
// --heap-pages=4096
// --json-file=benchmarking/results/results-pallet_asset_manager.json
// --pallet=pallet_asset_manager
// --chain=dev
// --output=benchmarking/new-benchmarks/pallet_asset_manager.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_asset_manager`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_asset_manager::WeightInfo for WeightInfo<T> {
	/// Storage: `AssetManager::AssetIdType` (r:1 w:1)
	/// Proof: `AssetManager::AssetIdType` (`max_values`: None, `max_size`: None, mode: `Measured`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(210), added: 2685, mode: `MaxEncodedLen`)
	/// Storage: `Assets::NextAssetId` (r:1 w:0)
	/// Proof: `Assets::NextAssetId` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Metadata` (r:1 w:1)
	/// Proof: `Assets::Metadata` (`max_values`: None, `max_size`: Some(140), added: 2615, mode: `MaxEncodedLen`)
	/// Storage: `AssetManager::AssetTypeId` (r:0 w:1)
	/// Proof: `AssetManager::AssetTypeId` (`max_values`: None, `max_size`: None, mode: `Measured`)
	fn register_foreign_asset() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `82`
		//  Estimated: `3675`
		// Minimum execution time: 40_882_000 picoseconds.
		Weight::from_parts(41_824_000, 0)
			.saturating_add(Weight::from_parts(0, 3675))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	/// Storage: `AssetManager::AssetIdType` (r:1 w:1)
	/// Proof: `AssetManager::AssetIdType` (`max_values`: None, `max_size`: None, mode: `Measured`)
	/// Storage: `AssetManager::AssetTypeId` (r:0 w:2)
	/// Proof: `AssetManager::AssetTypeId` (`max_values`: None, `max_size`: None, mode: `Measured`)
	fn change_existing_asset_type() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `429 + x * (4 ±0)`
		//  Estimated: `3870 + x * (5 ±0)`
		// Minimum execution time: 22_029_000 picoseconds.
		Weight::from_parts(25_957_100, 0)
			.saturating_add(Weight::from_parts(0, 3870))
			// Standard Error: 2_899
			.saturating_add(Weight::from_parts(93_865, 0))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
			.saturating_add(Weight::from_parts(0, 5))
	}
	/// Storage: `AssetManager::AssetIdType` (r:1 w:1)
	/// Proof: `AssetManager::AssetIdType` (`max_values`: None, `max_size`: None, mode: `Measured`)
	/// Storage: `AssetManager::AssetTypeId` (r:0 w:1)
	/// Proof: `AssetManager::AssetTypeId` (`max_values`: None, `max_size`: None, mode: `Measured`)
	fn remove_existing_asset_type() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `429 + x * (4 ±0)`
		//  Estimated: `3870 + x * (5 ±0)`
		// Minimum execution time: 19_492_000 picoseconds.
		Weight::from_parts(23_110_928, 0)
			.saturating_add(Weight::from_parts(0, 3870))
			// Standard Error: 2_937
			.saturating_add(Weight::from_parts(89_796, 0))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(2))
			.saturating_add(Weight::from_parts(0, 5))
	}
}
