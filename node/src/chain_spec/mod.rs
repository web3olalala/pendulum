use cumulus_primitives_core::ParaId;
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::{Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

pub mod amplitude;
pub mod pendulum_testnet;
