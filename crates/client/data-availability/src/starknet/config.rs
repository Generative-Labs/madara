use std::fs::File;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::DaMode;

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct StarknetConfig {
    pub http_provider: String,
    pub core_contracts: String,
    pub sequencer_key: String,
    pub account_address: String,
    pub chain_id: String,
    pub mode: DaMode,
    pub poll_interval_ms: Option<u64>,
}

impl TryFrom<&PathBuf> for StarknetConfig {
    type Error = String;

    fn try_from(path: &PathBuf) -> Result<Self, Self::Error> {
        let file = File::open(path).map_err(|e| format!("error opening da config: {e}"))?;
        serde_json::from_reader(file).map_err(|e| format!("error parsing da config: {e}"))
    }
}
