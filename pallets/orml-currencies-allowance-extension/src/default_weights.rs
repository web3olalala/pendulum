
//! Autogenerated weights for orml_currencies_allowance_extension
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-03-20, STEPS: `100`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `MacBook-Pro`, CPU: `<UNKNOWN>`
//! EXECUTION: None, WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/spacewalk-standalone
// benchmark
// pallet
// --chain=dev
// --pallet=orml-currencies-allowance-extension
// --extrinsic=*
// --steps=100
// --repeat=10
// --wasm-execution=compiled
// --output=pallets/default_weights.rs
// --template=./.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for orml_currencies_allowance_extension.
pub trait WeightInfo {
	fn add_allowed_currencies() -> Weight;
	fn remove_allowed_currencies() -> Weight;
}

/// Weights for orml_currencies_allowance_extension using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: TokenAllowance AllowedCurrencies (r:0 w:1)
	fn add_allowed_currencies() -> Weight {
		// Minimum execution time: 12_000 nanoseconds.
		Weight::from_parts(14_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: TokenAllowance AllowedCurrencies (r:0 w:1)
	fn remove_allowed_currencies() -> Weight {
		// Minimum execution time: 31_000 nanoseconds.
		Weight::from_parts(32_000_000 as u64, 0)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: TokenAllowance AllowedCurrencies (r:0 w:1)
	fn add_allowed_currencies() -> Weight {
		// Minimum execution time: 12_000 nanoseconds.
		Weight::from_parts(14_000_000 as u64, 0)
			.saturating_add(RocksDbWeight::get().writes(1 as u64))
	}
	// Storage: TokenAllowance AllowedCurrencies (r:0 w:1)
	fn remove_allowed_currencies() -> Weight {
		// Minimum execution time: 31_000 nanoseconds.
		Weight::from_parts(32_000_000 as u64, 0)
			.saturating_add(RocksDbWeight::get().writes(1 as u64))
	}
}
