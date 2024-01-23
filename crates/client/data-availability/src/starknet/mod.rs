pub mod config;

use std::collections::HashMap;
use std::ops::Add;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ethers::types::{I256, U256};
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
        let last_state = self
            .provider
            .call(
                FunctionCall {
                    contract_address: self.da_contract,
                    entry_point_selector: get_selector_from_name("LastState")?,
                    calldata: vec![],
                },
                BlockId::Tag(BlockTag::Latest),
            )
            .await
            .map_err(|e| anyhow!("call contract has error: {:?}", e))?;
        if last_state.len() != 6 {
            return Ok(I256::zero());
        }

        // In The current DA, the `last_published_state` ABI is as follows:
        // ```LastState(self: @ContractState) -> (u256, u256, u256)```.
        // The second parameter in the ABI represents the last published
        // block number. Since in the Cairo contract, a `u256` is composed of two `u128` values, we
        // extract the third element from the returned data as the last published block number.
        last_state[2]
            .try_into()
            .map(|n: u128| I256::from(n))
            .map_err(|_e| anyhow!("last published block number exceed i256 max"))
    }

    async fn publish_state_diff(&self, state_diff: Vec<U256>) -> Result<()> {
        let state_diff_len = state_diff.len();
        if state_diff_len < 3 {
            return Err(anyhow!("invalid state diff"));
        }

        // In the current contract, the ABI for publishing the state diff is defined as follows:
        // ```cairo
        //  fn UpdateState(
        //       ref self: ContractState,
        //       state_diff: Array<u256>,
        //       state_root: u256,
        //       block_number: u256,
        //       block_hash: u256
        //  ) -> bool
        // ```
        // In the Cairo contract, a `u256` is passed as two `u128` values.
        // Additionally, the `Array` parameter requires passing the length at the beginning.
        // Therefore, the overall calldata length is 1 + state_diff * 2 + 3 * 2.
        let mut calldata = Vec::with_capacity(state_diff_len * 2 + 1);

        // push state diff data len to calldata.
        calldata.push(FieldElement::from(state_diff_len - 3));
        for (index, ft) in state_diff.iter().enumerate() {
            // TODO: set current block number to l2 last published block number. just for testnet.
            if index == state_diff_len - 2 {
                let last_published_block_number = U256::from(self.last_published_state().await?.as_u128());
                let current_block_number = last_published_block_number.add(U256::one());
                let low_ft: u128 = current_block_number.as_u128();
                let high_ft: u128 = (current_block_number >> 128).as_u128();
                calldata.push(FieldElement::from(low_ft));
                calldata.push(FieldElement::from(high_ft));
                continue;
            }

            let low_ft: u128 = ft.as_u128();
            let high_ft: u128 = (ft >> 128).as_u128();
            calldata.push(FieldElement::from(low_ft));
            calldata.push(FieldElement::from(high_ft));
        }

        let _ = self
            .sequencer_account
            .execute(vec![Call {
                to: self.da_contract,
                selector: get_selector_from_name("UpdateState").map_err(|e| anyhow!("get selector failed, {e}"))?,
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

        let chain_id =
            FieldElement::from_byte_slice_be(conf.chain_id.as_bytes()).map_err(|e| format!("invalid chain id {e}"))?;

        let provider = create_provider(&conf.http_provider)?;
        let account = FieldElement::from_hex_be(&conf.account_address)
            .map(|acc| SingleOwnerAccount::new(provider, signer, acc, chain_id, ExecutionEncoding::New))
            .map_err(|e| format!("invalid sequencer address {e}"))?;

        Ok(Self {
            provider: create_provider(&conf.http_provider)?,
            da_contract,
            sequencer_account: account,
            mode: conf.mode,
        })
    }
}
