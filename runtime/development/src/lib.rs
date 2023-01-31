#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

mod weights;
pub mod xcm_config;

use codec::Encode;
pub use dia_oracle::dia::*;

use smallvec::smallvec;
use sp_api::impl_runtime_apis;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata, H256};
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{AccountIdConversion, AccountIdLookup, BlakeTwo256, Block as BlockT, Convert},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, DispatchError, FixedPointNumber, SaturatedConversion,
};

use sp_std::{marker::PhantomData, prelude::*};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use frame_support::{
	construct_runtime,
	dispatch::DispatchClass,
	parameter_types,
	traits::{ConstU32, Contains, Everything},
	weights::{
		constants::WEIGHT_REF_TIME_PER_SECOND, ConstantMultiplier, Weight, WeightToFeeCoefficient,
		WeightToFeeCoefficients, WeightToFeePolynomial,
	},
	PalletId,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot,
};
pub use sp_runtime::{MultiAddress, Perbill, Permill, Perquintill};

use runtime_common::{opaque, AuraId, Index, EXISTENTIAL_DEPOSIT, MICROUNIT, MILLIUNIT, NANOUNIT};

use cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases;

use xcm_config::{XcmConfig, XcmOriginToTransactDispatchOrigin};

use currency::Amount;
use orml_traits::{currency::MutationHooks, parameter_type_with_key};

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

pub use issue::{Event as IssueEvent, IssueRequest};
pub use nomination::Event as NominationEvent;
pub use redeem::{Event as RedeemEvent, RedeemRequest};
pub use replace::{Event as ReplaceEvent, ReplaceRequest};
pub use security::StatusCode;
pub use stellar_relay::traits::{FieldLength, Organization, Validator};

pub use module_oracle_rpc_runtime_api::BalanceWrapper;
use oracle::dia::DiaOracleAdapter;

// Polkadot imports
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};

use spacewalk_primitives::{
	self as primitives, AccountId, Balance, BlockNumber, CurrencyId, CurrencyId::XCM,
	ForeignCurrencyId, Hash, Moment, Signature, SignedFixedPoint, SignedInner, UnsignedFixedPoint,
	UnsignedInner,
};

use weights::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight};

// XCM Imports
use xcm::latest::prelude::BodyId;
use xcm_executor::XcmExecutor;

pub type VaultId = primitives::VaultId<AccountId, CurrencyId>;

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;

/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
>;

type DataProviderImpl = DiaOracleAdapter<
	DiaOracle,
	UnsignedFixedPoint,
	Moment,
	primitives::DiaOracleKeyConvertor,
	ConvertPrice,
	ConvertMoment,
>;

pub struct ConvertPrice;
impl Convert<u128, Option<UnsignedFixedPoint>> for ConvertPrice {
	fn convert(price: u128) -> Option<UnsignedFixedPoint> {
		Some(UnsignedFixedPoint::from_inner(price))
	}
}

pub struct ConvertMoment;
impl Convert<u64, Option<Moment>> for ConvertMoment {
	fn convert(moment: u64) -> Option<Moment> {
		Some(moment)
	}
}

impl frame_system::offchain::SigningTypes for Runtime {
	type Public = <Signature as sp_runtime::traits::Verify>::Signer;
	type Signature = Signature;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
	RuntimeCall: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: RuntimeCall,
		public: <Signature as sp_runtime::traits::Verify>::Signer,
		account: AccountId,
		index: Index,
	) -> Option<(
		RuntimeCall,
		<UncheckedExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
	)> {
		let period = BlockHashCount::get() as u64;
		let current_block = System::block_number().saturated_into::<u64>().saturating_sub(1);
		let tip = 0;
		let extra: SignedExtra = (
			frame_system::CheckNonZeroSender::<Runtime>::new(),
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
			frame_system::CheckNonce::<Runtime>::from(index),
			frame_system::CheckWeight::<Runtime>::new(),
			pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
		);

		let raw_payload = SignedPayload::new(call, extra).ok()?;
		let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
		let address = account;
		let (call, extra, _) = raw_payload.deconstruct();
		Some((call, (sp_runtime::MultiAddress::Id(address), signature.into(), extra)))
	}
}

/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
/// node's balance type.
///
/// This should typically create a mapping between the following ranges:
///   - `[0, MAXIMUM_BLOCK_WEIGHT]`
///   - `[Balance::min, Balance::max]`
///
/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
///   - Setting it to `0` will essentially disable the weight fee.
///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
	type Balance = Balance;
	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		let p = MILLIUNIT / 10;
		let q = 100 * Balance::from(ExtrinsicBaseWeight::get().ref_time());
		smallvec![WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational(p % q, q),
			coeff_integer: p / q,
		}]
	}
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("pendulum-development"),
	impl_name: create_runtime_str!("pendulum-development"),
	authoring_version: 1,
	spec_version: 1,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	state_version: 1,
};

/// This determines the average expected block time that we are targeting.
/// Blocks will be produced at a minimum duration defined by `SLOT_DURATION`.
/// `SLOT_DURATION` is picked up by `pallet_timestamp` which is in turn picked
/// up by `pallet_aura` to implement `fn slot_duration()`.
///
/// Change this to adjust the block time.
pub const MILLISECS_PER_BLOCK: u64 = 12000;

// NOTE: Currently it is not possible to change the slot duration after the chain has started.
//       Attempting to do so will brick block production.
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;
pub const BLOCKS_PER_YEAR: BlockNumber = DAYS * 36525 / 100;

/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
/// used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
/// `Operational` extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// We allow for 0.5 of a second of compute with a 12 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight =
	Weight::from_ref_time(WEIGHT_REF_TIME_PER_SECOND.saturating_div(2))
		.set_proof_size(cumulus_primitives_core::relay_chain::v2::MAX_POV_SIZE as u64);

// todo: https://github.com/pendulum-chain/pendulum/pull/91#discussion_r1073402063
// For mainnet USDC issued by centre.io
// const WRAPPED_CURRENCY_ID: CurrencyId = CurrencyId::AlphaNum4 {
// 	code: *b"USDC",
// 	issuer: [
// 		59, 153, 17, 56, 14, 254, 152, 139, 160, 168, 144, 14, 177, 207, 228, 79, 54, 111, 125,
// 		190, 148, 107, 237, 7, 114, 64, 247, 246, 36, 223, 21, 197,
// 	],
// };
// For Testnet USDC issued by
const WRAPPED_CURRENCY_ID: CurrencyId = CurrencyId::AlphaNum4 {
	code: *b"USDC",
	issuer: [
		20, 209, 150, 49, 176, 55, 23, 217, 171, 154, 54, 110, 16, 50, 30, 226, 102, 231, 46, 199,
		108, 171, 97, 144, 240, 161, 51, 109, 72, 34, 159, 139,
	],
};

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;

	// This part is copied from Substrate's `bin/node/runtime/src/lib.rs`.
	//  The `RuntimeBlockLength` and `RuntimeBlockWeights` exist here because the
	// `DeletionWeightLimit` and `DeletionQueueDepth` depend on those to parameterize
	// the lazy contract deletion.
	pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub const SS58Prefix: u16 = 42;
}

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The aggregated dispatch type that is available for extrinsics.
	type RuntimeCall = RuntimeCall;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = AccountIdLookup<AccountId, ()>;
	/// The index type for storing how many extrinsics an account has signed.
	type Index = Index;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The header type.
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	/// The ubiquitous origin type.
	type RuntimeOrigin = RuntimeOrigin;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// Runtime version.
	type Version = Version;
	/// Converts a module to an index of this module in the runtime.
	type PalletInfo = PalletInfo;
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// What to do if a new account is created.
	type OnNewAccount = ();
	/// What to do if an account is fully reaped from the system.
	type OnKilledAccount = ();
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// The basic call filter to use in dispatchable.
	type BaseCallFilter = Everything;
	/// Weight information for the extrinsics of this pallet.
	type SystemWeightInfo = ();
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	/// The action to take on a Runtime Upgrade
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const UncleGenerations: u32 = 0;
}

impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = (CollatorSelection,);
}

parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = MaxLocks;
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
}

parameter_types! {
	/// Relay Chain `TransactionByteFee` / 10
	pub const TransactionByteFee: Balance = 10 * MICROUNIT;
	pub const OperationalFeeMultiplier: u8 = 5;
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = pallet_transaction_payment::CurrencyAdapter<Balances, ()>;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
}

parameter_types! {
	pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
}

impl cumulus_pallet_parachain_system::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnSystemEvent = ();
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type DmpMessageHandler = DmpQueue;
	type ReservedDmpWeight = ReservedDmpWeight;
	type OutboundXcmpMessageSource = XcmpQueue;
	type XcmpMessageHandler = XcmpQueue;
	type ReservedXcmpWeight = ReservedXcmpWeight;
	type CheckAssociatedRelayNumber = RelayNumberStrictlyIncreases;
}

impl parachain_info::Config for Runtime {}

impl cumulus_pallet_aura_ext::Config for Runtime {}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = ();
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
	type ControllerOrigin = EnsureRoot<AccountId>;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type WeightInfo = ();
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
}

parameter_types! {
	pub const Period: u32 = 6 * HOURS;
	pub const Offset: u32 = 0;
	pub const MaxAuthorities: u32 = 100_000;
}

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = CollatorSelection;
	// Essentially just Aura, but lets be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type WeightInfo = pallet_session::weights::SubstrateWeight<Runtime>;
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = MaxAuthorities;
}

parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
	pub const MaxCandidates: u32 = 1000;
	pub const MinCandidates: u32 = 5;
	pub const SessionLength: BlockNumber = 6 * HOURS;
	pub const MaxInvulnerables: u32 = 100;
	pub const ExecutiveBody: BodyId = BodyId::Executive;
}

// We allow root only to execute privileged collator selection operations.
pub type CollatorSelectionUpdateOrigin = EnsureRoot<AccountId>;

impl pallet_collator_selection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type UpdateOrigin = CollatorSelectionUpdateOrigin;
	type PotId = PotId;
	type MaxCandidates = MaxCandidates;
	type MinCandidates = MinCandidates;
	type MaxInvulnerables = MaxInvulnerables;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = Session;
	type WeightInfo = pallet_collator_selection::weights::SubstrateWeight<Runtime>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		NANOUNIT
	};
}

pub struct DustRemovalWhitelist;
impl Contains<AccountId> for DustRemovalWhitelist {
	fn contains(_a: &AccountId) -> bool {
		true
	}
}

pub struct CurrencyHooks<T>(PhantomData<T>);
impl<T: orml_tokens::Config> MutationHooks<T::AccountId, T::CurrencyId, T::Balance>
	for CurrencyHooks<T>
{
	type OnDust = orml_tokens::BurnDust<T>;
	type OnSlash = ();
	type PreDeposit = ();
	type PostDeposit = ();
	type PreTransfer = ();
	type PostTransfer = ();
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = primitives::Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = CurrencyHooks<Runtime>;
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ConstU32<0>;
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = DustRemovalWhitelist;
}

pub struct CurrencyConvert;
impl currency::CurrencyConversion<currency::Amount<Runtime>, CurrencyId> for CurrencyConvert {
	fn convert(
		amount: &currency::Amount<Runtime>,
		to: CurrencyId,
	) -> Result<currency::Amount<Runtime>, DispatchError> {
		Oracle::convert(amount, to)
	}
}

parameter_types! {
	pub const RelayChainCurrencyId: CurrencyId = XCM(ForeignCurrencyId::KSM);
	pub const NativeCurrencyId: CurrencyId = CurrencyId::Native;
	pub const DefaultWrappedCurrencyId: CurrencyId = WRAPPED_CURRENCY_ID;
}
impl currency::Config for Runtime {
	type UnsignedFixedPoint = UnsignedFixedPoint;
	type SignedInner = SignedInner;
	type SignedFixedPoint = SignedFixedPoint;
	type Balance = Balance;
	type GetNativeCurrencyId = NativeCurrencyId;
	type GetRelayChainCurrencyId = RelayChainCurrencyId;
	type AssetConversion = primitives::AssetConversion;
	type BalanceConversion = primitives::BalanceConversion;
	type CurrencyConversion = CurrencyConvert;
}

impl security::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = security::SubstrateWeight<Runtime>;
}

impl staking::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SignedInner = SignedInner;
	type SignedFixedPoint = SignedFixedPoint;
	type GetNativeCurrencyId = NativeCurrencyId;
	type CurrencyId = CurrencyId;
}

impl oracle::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = oracle::SubstrateWeight<Runtime>;
	type DataProvider = DataProviderImpl;
}

parameter_types! {
	pub const OrganizationLimit: u32 = 255;
	pub const ValidatorLimit: u32 = 255;
}

impl stellar_relay::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OrganizationId = u128;
	type OrganizationLimit = OrganizationLimit;
	type ValidatorLimit = ValidatorLimit;
	type WeightInfo = ();
}

impl reward::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SignedFixedPoint = SignedFixedPoint;
	type RewardId = VaultId;
	type CurrencyId = CurrencyId;
	type GetNativeCurrencyId = NativeCurrencyId;
}

parameter_types! {
	pub const FeePalletId: PalletId = PalletId(*b"mod/fees");
	pub const VaultRegistryPalletId: PalletId = PalletId(*b"mod/vreg");

	pub const MaxExpectedValue: UnsignedFixedPoint = UnsignedFixedPoint::from_inner(<UnsignedFixedPoint as FixedPointNumber>::DIV);
	pub FeeAccount: AccountId = FeePalletId::get().into_account_truncating();
}

impl fee::Config for Runtime {
	type FeePalletId = FeePalletId;
	type WeightInfo = fee::SubstrateWeight<Runtime>;
	type SignedFixedPoint = SignedFixedPoint;
	type SignedInner = SignedInner;
	type UnsignedFixedPoint = UnsignedFixedPoint;
	type UnsignedInner = UnsignedInner;
	type VaultRewards = VaultRewards;
	type VaultStaking = VaultStaking;
	type OnSweep = currency::SweepFunds<Runtime, FeeAccount>;
	type MaxExpectedValue = MaxExpectedValue;
}

impl vault_registry::Config for Runtime {
	type PalletId = VaultRegistryPalletId;
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type WeightInfo = vault_registry::SubstrateWeight<Runtime>;
	type GetGriefingCollateralCurrencyId = NativeCurrencyId;
}

impl redeem::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = redeem::SubstrateWeight<Runtime>;
}

pub struct BlockNumberToBalance;

impl sp_runtime::traits::Convert<BlockNumber, Balance> for BlockNumberToBalance {
	fn convert(a: BlockNumber) -> Balance {
		a.into()
	}
}

impl issue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type BlockNumberToBalance = BlockNumberToBalance;
	type WeightInfo = issue::SubstrateWeight<Runtime>;
}

impl nomination::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = nomination::SubstrateWeight<Runtime>;
}

impl replace::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = replace::SubstrateWeight<Runtime>;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
	RuntimeCall: From<C>,
{
	type Extrinsic = UncheckedExtrinsic;
	type OverarchingCall = RuntimeCall;
}
impl dia_oracle::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type AuthorityId = dia_oracle::crypto::DiaAuthId;
	type WeightInfo = dia_oracle::weights::DiaWeightInfo<Runtime>;
}
// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		// System support stuff.
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
		ParachainSystem: cumulus_pallet_parachain_system::{
			Pallet, Call, Config, Storage, Inherent, Event<T>, ValidateUnsigned,
		} = 1,
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent} = 2,
		ParachainInfo: parachain_info::{Pallet, Storage, Config} = 3,

		// Monetary stuff.
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>} = 10,
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>} = 11,

		// Collator support. The order of these 4 are important and shall not change.
		Authorship: pallet_authorship::{Pallet, Call, Storage} = 20,
		CollatorSelection: pallet_collator_selection::{Pallet, Call, Storage, Event<T>, Config<T>} = 21,
		Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>} = 22,
		Aura: pallet_aura::{Pallet, Storage, Config<T>} = 23,
		AuraExt: cumulus_pallet_aura_ext::{Pallet, Storage, Config} = 24,

		// XCM helpers.
		XcmpQueue: cumulus_pallet_xcmp_queue::{Pallet, Call, Storage, Event<T>} = 30,
		PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin, Config} = 31,
		CumulusXcm: cumulus_pallet_xcm::{Pallet, Event<T>, Origin} = 32,
		DmpQueue: cumulus_pallet_dmp_queue::{Pallet, Call, Storage, Event<T>} = 33,

		// Currency pallet required this.
		Tokens: orml_tokens::{Pallet, Call, Storage, Config<T>, Event<T>} = 53,

		Security: security::{Pallet, Call, Config, Storage, Event<T>} = 55,
		VaultStaking: staking::{Pallet, Storage, Event<T>} = 56,
		Oracle: oracle::{Pallet, Call, Config, Storage, Event<T>} = 57,
		Currency: currency::{Pallet} = 58,
		StellarRelay: stellar_relay::{Pallet, Call, Config<T>, Storage, Event<T>} = 59,
		VaultRewards: reward::{Pallet, Call, Storage, Event<T>} = 60,
		Fee: fee::{Pallet, Call, Config<T>, Storage} = 61,
		VaultRegistry: vault_registry::{Pallet, Call, Config<T>, Storage, Event<T>, ValidateUnsigned} = 62,
		Redeem: redeem::{Pallet, Call, Config<T>, Storage, Event<T>} = 63,
		Issue: issue::{Pallet, Call, Config<T>, Storage, Event<T>} = 64,
		Nomination: nomination::{Pallet, Call, Config, Storage, Event<T>} = 65,
		DiaOracle: dia_oracle::{Pallet, Call, Config<T>, Storage, Event<T>} = 66,
		Replace: replace::{Pallet, Call, Config<T>, Storage, Event<T>} = 67,
	}
);

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate frame_benchmarking;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	define_benchmarks!(
		[frame_system, SystemBench::<Runtime>]
		[pallet_balances, Balances]
		[pallet_session, SessionBench::<Runtime>]
		[pallet_timestamp, Timestamp]
		[pallet_collator_selection, CollatorSelection]
		[cumulus_pallet_xcmp_queue, XcmpQueue]
		[oracle, Oracle]
		[stellar_relay, StellarRelay]
		[fee, Fee]
		[vault_registry, VaultRegistry]
		[redeem, Redeem]
		[issue, Issue]
		[nomination, Nomination]
		[replace, Replace]
	);
}

impl_runtime_apis! {
	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			Aura::authorities().into_inner()
		}
	}

	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade() -> (Weight, Weight) {
			log::info!("try-runtime::on_runtime_upgrade pendulum-development.");
			let weight = Executive::try_runtime_upgrade().unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block_no_check(block: Block) -> Weight {
			Executive::execute_block_no_check(block)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use frame_system_benchmarking::Pallet as SystemBench;
			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			return (list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, TrackedStorageKey};

			use frame_system_benchmarking::Pallet as SystemBench;
			impl frame_system_benchmarking::Config for Runtime {}

			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
			impl cumulus_pallet_session_benchmarking::Config for Runtime {}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}

	impl module_issue_rpc_runtime_api::IssueApi<
		Block,
		AccountId,
		H256,
		IssueRequest<AccountId, BlockNumber, Balance, CurrencyId>
	> for Runtime {
		fn get_issue_requests(account_id: AccountId) -> Vec<H256> {
			Issue::get_issue_requests_for_account(account_id)
		}

		fn get_vault_issue_requests(vault_id: AccountId) -> Vec<H256> {
			Issue::get_issue_requests_for_vault(vault_id)
		}
	}

	impl module_vault_registry_rpc_runtime_api::VaultRegistryApi<
		Block,
		VaultId,
		Balance,
		UnsignedFixedPoint,
		CurrencyId,
		AccountId,
	> for Runtime {
		fn get_vault_collateral(vault_id: VaultId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let result = VaultRegistry::compute_collateral(&vault_id)?;
			Ok(BalanceWrapper{amount:result.amount()})
		}

		fn get_vaults_by_account_id(account_id: AccountId) -> Result<Vec<VaultId>, DispatchError> {
			VaultRegistry::get_vaults_by_account_id(account_id)
		}

		fn get_vault_total_collateral(vault_id: VaultId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let result = VaultRegistry::get_backing_collateral(&vault_id)?;
			Ok(BalanceWrapper{amount:result.amount()})
		}

		fn get_premium_redeem_vaults() -> Result<Vec<(VaultId, BalanceWrapper<Balance>)>, DispatchError> {
			let result = VaultRegistry::get_premium_redeem_vaults()?;
			Ok(result.iter().map(|v| (v.0.clone(), BalanceWrapper{amount:v.1.amount()})).collect())
		}

		fn get_vaults_with_issuable_tokens() -> Result<Vec<(VaultId, BalanceWrapper<Balance>)>, DispatchError> {
			let result = VaultRegistry::get_vaults_with_issuable_tokens()?;
			Ok(result.into_iter().map(|v| (v.0, BalanceWrapper{amount:v.1.amount()})).collect())
		}

		fn get_vaults_with_redeemable_tokens() -> Result<Vec<(VaultId, BalanceWrapper<Balance>)>, DispatchError> {
			let result = VaultRegistry::get_vaults_with_redeemable_tokens()?;
			Ok(result.into_iter().map(|v| (v.0, BalanceWrapper{amount:v.1.amount()})).collect())
		}

		fn get_issuable_tokens_from_vault(vault: VaultId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let result = VaultRegistry::get_issuable_tokens_from_vault(&vault)?;
			Ok(BalanceWrapper{amount:result.amount()})
		}

		fn get_collateralization_from_vault(vault: VaultId, only_issued: bool) -> Result<UnsignedFixedPoint, DispatchError> {
			VaultRegistry::get_collateralization_from_vault(vault, only_issued)
		}

		fn get_collateralization_from_vault_and_collateral(vault: VaultId, collateral: BalanceWrapper<Balance>, only_issued: bool) -> Result<UnsignedFixedPoint, DispatchError> {
			let amount = Amount::new(collateral.amount, vault.collateral_currency());
			VaultRegistry::get_collateralization_from_vault_and_collateral(vault, &amount, only_issued)
		}

		fn get_required_collateral_for_wrapped(amount_wrapped: BalanceWrapper<Balance>, currency_id: CurrencyId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let amount_wrapped = Amount::new(amount_wrapped.amount, DefaultWrappedCurrencyId::get());
			let result = VaultRegistry::get_required_collateral_for_wrapped(&amount_wrapped, currency_id)?;
			Ok(BalanceWrapper{amount:result.amount()})
		}

		fn get_required_collateral_for_vault(vault_id: VaultId) -> Result<BalanceWrapper<Balance>, DispatchError> {
			let result = VaultRegistry::get_required_collateral_for_vault(vault_id)?;
			Ok(BalanceWrapper{amount:result.amount()})
		}
	}

	impl module_redeem_rpc_runtime_api::RedeemApi<
		Block,
		AccountId,
		H256,
		RedeemRequest<AccountId, BlockNumber, Balance, CurrencyId>
	> for Runtime {
		fn get_redeem_requests(account_id: AccountId) -> Vec<H256> {
			Redeem::get_redeem_requests_for_account(account_id)
		}

		fn get_vault_redeem_requests(vault_account_id: AccountId) -> Vec<H256> {
			Redeem::get_redeem_requests_for_vault(vault_account_id)
		}
	}

	impl module_replace_rpc_runtime_api::ReplaceApi<
		Block,
		AccountId,
		H256,
		ReplaceRequest<AccountId, BlockNumber, Balance, CurrencyId>
	> for Runtime {
		fn get_old_vault_replace_requests(vault_id: AccountId) -> Vec<H256> {
			Replace::get_replace_requests_for_old_vault(vault_id)
		}

		fn get_new_vault_replace_requests(vault_id: AccountId) -> Vec<H256> {
			Replace::get_replace_requests_for_new_vault(vault_id)
		}
	}

}

struct CheckInherents;

impl cumulus_pallet_parachain_system::CheckInherents<Block> for CheckInherents {
	fn check_inherents(
		block: &Block,
		relay_state_proof: &cumulus_pallet_parachain_system::RelayChainStateProof,
	) -> sp_inherents::CheckInherentsResult {
		let relay_chain_slot = relay_state_proof
			.read_slot()
			.expect("Could not read the relay chain slot from the proof");

		let inherent_data =
			cumulus_primitives_timestamp::InherentDataProvider::from_relay_chain_slot_and_duration(
				relay_chain_slot,
				sp_std::time::Duration::from_secs(6),
			)
			.create_inherent_data()
			.expect("Could not create the timestamp inherent data");

		inherent_data.check_extrinsics(block)
	}
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
	CheckInherents = CheckInherents,
}
