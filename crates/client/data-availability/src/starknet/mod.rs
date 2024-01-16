pub mod config;

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ethers::types::{I256, U256};
use mp_felt::Felt252Wrapper;
use reqwest::Url;
use starknet_accounts::{Account, Call, ExecutionEncoding, SingleOwnerAccount};
use starknet_core::types::{BlockId, BlockTag, FunctionCall};
use starknet_core::utils::get_selector_from_name;
use starknet_ff::FieldElement;
use starknet_providers::jsonrpc::HttpTransport;
use starknet_providers::{JsonRpcClient, Provider};
use starknet_signers::{LocalWallet, SigningKey};

use crate::{DaClient, DaMode};

#[derive(Debug)]
pub struct StarknetClient {
    provider: JsonRpcClient<HttpTransport>,
    da_contract: FieldElement,
    sequencer_account: SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
    mode: DaMode,
}

#[async_trait]
impl DaClient for StarknetClient {
    fn get_mode(&self) -> DaMode {
        self.mode
    }

    async fn last_published_state(&self) -> Result<I256> {
        self.provider
            .call(
                FunctionCall {
                    contract_address: self.da_contract,
                    entry_point_selector: get_selector_from_name("lastPublishedState")?,
                    calldata: vec![],
                },
                BlockId::Tag(BlockTag::Latest),
            )
            .await
            .map_err(|e| anyhow!("call contract has error: {:?}", e))?
            .first()
            .and_then(|e| u128::try_from(*e).ok())
            .and_then(|e| Some(I256::from(e)))
            .ok_or(anyhow!("invalid call contract result"))
    }

    async fn publish_state_diff(&self, state_diff: Vec<U256>) -> Result<()> {
        let calldata: Result<Vec<FieldElement>, _> =
            state_diff.iter().map(|d| Felt252Wrapper::try_from(*d).map(|fw| fw.0)).collect();

        let calldata = calldata?;

        let _ = self
            .sequencer_account
            .execute(vec![Call {
                to: self.da_contract,
                selector: get_selector_from_name("updateState").map_err(|e| anyhow!("get selector failed, {e}"))?,
                calldata,
            }])
            .send()
            .await
            .map_err(|e| anyhow!("send transaction failed {e}"))?;

        Ok(())
    }

    fn get_da_metric_labels(&self) -> HashMap<String, String> {
        [("name".into(), "starknet".into())].iter().cloned().collect()
    }
}

fn create_provider(url: &str) -> Result<JsonRpcClient<HttpTransport>, String> {
    Url::parse(url).map(|url| JsonRpcClient::new(HttpTransport::new(url))).map_err(|e| format!("invalid http url, {e}"))
}

impl TryFrom<config::StarknetConfig> for StarknetClient {
    type Error = String;

    fn try_from(conf: config::StarknetConfig) -> Result<Self, Self::Error> {
        let signer = FieldElement::from_hex_be(&conf.sequencer_key)
            .map(|elem| LocalWallet::from(SigningKey::from_secret_scalar(elem)))
            .map_err(|e| format!("invalid sequencer key, {e}"))?;

        let da_contract =
            FieldElement::from_hex_be(&conf.core_contracts).map_err(|e| format!("invalid da contract address, {e}"))?;

        let chain_id = FieldElement::from_hex_be(&conf.chain_id).map_err(|e| format!("invalid chain id {e}"))?;

        let provider = create_provider(&conf.http_provider)?;
        let account = FieldElement::from_hex_be(&conf.account_address)
            .map(|acc| SingleOwnerAccount::new(provider, signer, acc, chain_id, ExecutionEncoding::Legacy))
            .map_err(|e| format!("invalid sequencer address {e}"))?;

        Ok(Self {
            provider: create_provider(&conf.http_provider)?,
            da_contract,
            sequencer_account: account,
            mode: conf.mode,
        })
    }
}
