use cumulus_primitives_core::ParaId;
use pendulum_parachain_runtime::{AccountId, AuraId, Signature, EXISTENTIAL_DEPOSIT};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::UncheckedInto, ed25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

pub mod amplitude;
pub mod pendulum_testnet;

pub const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

pub type ChainSpec =
	sc_service::GenericChainSpec<pendulum_parachain_runtime::GenesisConfig, Extensions>;

pub type AccountPublic = <Signature as Verify>::Signer;

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Self> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}
