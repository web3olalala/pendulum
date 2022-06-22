use super::*;

pub const PARA_ID: u32 = 2124;

use pendulum_parachain_runtime::{AccountId, AuraId, Signature, EXISTENTIAL_DEPOSIT};
pub type ChainSpec =
	sc_service::GenericChainSpec<pendulum_parachain_runtime::GenesisConfig, Extensions>;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

/// Helper function to generate a crypto pair from seed
pub fn get_public_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

fn amplitude_properties() -> Map<String, Value> {
	let mut properties = sc_chain_spec::Properties::new();
	// FIXME maybe we need something else here
	properties.insert("tokenSymbol".into(), "AMPE".into());
	// check decimals here https://github.com/centrifuge/centrifuge-chain/blob/96484bcc4190483e05e59a66253701db728bd92f/runtime/common/src/lib.rs#L277
	properties.insert("tokenDecimals".into(), 12.into());
	// what exactly is this used for? centrifuge does not use it
	properties.insert("ss58Format".into(), 42.into());
	properties
}

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

type AccountPublic = <Signature as Verify>::Signer;

pub fn get_collator_keys_from_seed(seed: &str) -> AuraId {
	get_public_from_seed::<AuraId>(seed)
}

pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_public_from_seed::<TPublic>(seed)).into_account()
}

fn get_authority_keys_from_public_key(src: [u8; 32]) -> (AccountId, AuraId) {
	(src.clone().into(), src.unchecked_into())
}

pub fn generate_session_keys(keys: AuraId) -> pendulum_parachain_runtime::SessionKeys {
	pendulum_parachain_runtime::SessionKeys { aura: keys }
}

pub fn amplitude_local_config() -> ChainSpec {
	let id: ParaId = PARA_ID.into();

	ChainSpec::from_genesis(
		"Amplitude",
		"amplitude",
		ChainType::Local,
		move || pendulum_mainnet_genesis(vec![], vec![], id),
		Vec::new(),
		None,
		None,
		None,
		Some(amplitude_properties()),
		Extensions { relay_chain: "kusama".into(), para_id: id.into() },
	)
}
pub fn amplitude_mainnet_config() -> ChainSpec {
	let id: ParaId = PARA_ID.into();
	ChainSpec::from_genesis(
		"Amplitude",
		"amplitude",
		ChainType::Live,
		move || pendulum_mainnet_genesis(vec![], vec![], id),
		Vec::new(),
		None,
		None,
		None,
		Some(amplitude_properties()),
		Extensions { relay_chain: "kusama".into(), para_id: id.into() },
	)
}

fn pendulum_mainnet_genesis(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<AccountId>,
	id: ParaId,
) -> pendulum_parachain_runtime::GenesisConfig {
	pendulum_parachain_runtime::GenesisConfig {
		system: pendulum_parachain_runtime::SystemConfig {
			code: pendulum_parachain_runtime::WASM_BINARY
				.expect(
					"WASM binary was not build, please buildget_authority_keys_from_public_key it!",
				)
				.to_vec(),
		},
		balances: pendulum_parachain_runtime::BalancesConfig {
			balances: endowed_accounts.iter().cloned().map(|k| (k, 1 << 60)).collect(),
		},
		parachain_info: pendulum_parachain_runtime::ParachainInfoConfig { parachain_id: id },
		collator_selection: pendulum_parachain_runtime::CollatorSelectionConfig {
			invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect(),
			// TODO maybe change this.
			// But if we change it to above 0 make sure the invulnerables have enough balance to pay it
			candidacy_bond: EXISTENTIAL_DEPOSIT * 16,
			..Default::default()
		},
		session: pendulum_parachain_runtime::SessionConfig {
			keys: invulnerables
				.into_iter()
				.map(|(acc, aura)| {
					(
						acc.clone(),                 // account id
						acc,                         // validator id
						generate_session_keys(aura), // session keys
					)
				})
				.collect(),
		},
		// no need to pass anything to aura, in fact it will panic if we do. Session will take care
		// of this.
		aura: Default::default(),
		aura_ext: Default::default(),
		parachain_system: Default::default(),
		polkadot_xcm: pendulum_parachain_runtime::PolkadotXcmConfig {
			safe_xcm_version: Some(SAFE_XCM_VERSION),
		},
	}
}
