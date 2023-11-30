use std::result::Result;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use ethers::abi::RawLog;
use ethers::contract::{BaseContract, EthEvent, EthLogDecode};
use ethers::core::abi::parse_abi;
#[cfg(test)]
use ethers::providers::MockProvider;
use ethers::providers::{Http, JsonRpcClient, Middleware, Provider};
use ethers::types::{Address, Filter, Log, H256, I256, U256};
use log::debug;
use sp_runtime::generic::BlockId;
use sp_runtime::traits::Block as BlockT;
use starknet_api::state::StateDiff;
use tokio::time::sleep;

use super::*;

const STATE_SEARCH_STEP: u64 = 10;
const LOG_SEARCH_STEP: u64 = 1000;

#[derive(Debug)]
pub struct EthOrigin {
    block_hash: H256,
    block_number: u64,
    _transaction_hash: H256,
    transaction_index: u64,
}

#[derive(Debug)]
pub struct StateUpdate {
    eth_origin: EthOrigin,
    update: LogStateUpdate,
}

#[derive(Clone, Debug, PartialEq, Eq, EthEvent)]
#[ethevent(name = "LogStateUpdate")]
pub struct LogStateUpdate {
    pub global_root: U256,
    pub block_number: I256,
    pub block_hash: U256,
}

#[derive(Clone, Debug, PartialEq, Eq, EthEvent)]
#[ethevent(name = "LogStateTransitionFact")]
pub struct LogStateTransitionFact {
    pub fact: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, EthEvent)]
#[ethevent(name = "LogMemoryPagesHashes")]
pub struct LogMemoryPagesHashes {
    pub fact: [u8; 32],
    pub pages_hashes: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, PartialEq, Eq, EthEvent)]
#[ethevent(name = "LogMemoryPageFactContinuous")]
pub struct LogMemoryPageFactContinuous {
    pub fact_hash: [u8; 32],
    pub memory_hash: U256,
    pub prod: U256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogMemoryPageFactContinuousWithTxHash {
    pub log_memory_page_fact_continuous: LogMemoryPageFactContinuous,
    pub tx_hash: H256,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self::SYNCING
    }
}

#[derive(Debug, Clone)]
pub struct EthereumStateFetcher<P: JsonRpcClient> {
    http_provider: Provider<P>,

    core_contract: Address,

    verifier_contract: Address,

    memory_page_contract: Address,

    eth_url_list: Vec<String>, // Ethereum Node URL List

    current_provider_index: Arc<Mutex<usize>>,

    sync_status: Arc<Mutex<SyncStatus>>,

    v011_diff_format_height: u64,
}

impl EthereumStateFetcher<Http> {
    pub fn new(
        core_contract: Address,
        verifier_contract: Address,
        memory_page_contract: Address,
        eth_urls: Vec<String>,
        v011_diff_format_height: u64,
        sync_status: Arc<Mutex<SyncStatus>>,
    ) -> Result<Self, Error> {
        let provider =
            Provider::<Http>::try_from(eth_urls[0].clone()).map_err(|e| Error::L1Connection(e.to_string()))?;

        Ok(Self {
            http_provider: provider,
            core_contract,
            verifier_contract,
            memory_page_contract,
            eth_url_list: eth_urls,
            current_provider_index: Arc::new(Mutex::new(0)),
            sync_status,
            v011_diff_format_height,
        })
    }
}

#[cfg(test)]
impl EthereumStateFetcher<MockProvider> {
    pub fn mock(
        core_contract: Address,
        verifier_contract: Address,
        memory_page_contract: Address,
        eth_url_list: Vec<String>,
        v011_diff_format_height: u64,
        mock_provider: MockProvider,
        sync_status: Arc<Mutex<SyncStatus>>,
    ) -> Result<Self, Error> {
        let provider = Provider::<MockProvider>::new(mock_provider);

        Ok(Self {
            http_provider: provider,
            core_contract,
            verifier_contract,
            memory_page_contract,
            eth_url_list,
            current_provider_index: Arc::new(Mutex::new(0)),
            sync_status,
            v011_diff_format_height,
        })
    }
}

impl<P: JsonRpcClient + Clone> EthereumStateFetcher<P> {
    pub async fn get_logs_retry(&mut self, filter: &Filter) -> Result<Vec<Log>, Error> {
        if let Ok(res) = self.http_provider.get_logs(&filter).await {
            return Ok(res);
        }

        let mut retries = 0;
        loop {
            let provider = self
                .current_provider_index
                .lock()
                .map(|index| {
                    let current_provider_url = &self.eth_url_list[*index];
                    Provider::<Http>::try_from(current_provider_url).map_err(|e| Error::L1Connection(e.to_string()))
                })
                .map_err(|e| Error::Other(e.to_string()))??;

            match provider.get_logs(&filter).await {
                Ok(logs) => return Ok(logs),
                Err(_e) => {
                    retries += 1;
                    if retries > self.eth_url_list.len() {
                        return Err(Error::L1Connection("All Ethereum nodes failed.".to_string()));
                    }

                    // change to next Ethereum node
                    if let Ok(mut index) = self.current_provider_index.lock() {
                        *index = (*index + 1) % self.eth_url_list.len();
                    };

                    // Calculate the wait time manually
                    let wait_time = self.calculate_backoff(retries);

                    sleep(wait_time).await;
                }
            }
        }
    }

    // Custom backoff calculation
    pub fn calculate_backoff(&self, retries: usize) -> Duration {
        // A simple exponential backoff with a maximum delay of 10 seconds
        let base_delay = 1.0; // in seconds
        let max_delay = 10; // in seconds
        let exponential_factor: f64 = 2.0;

        // Calculate the backoff using the exponential factor
        let backoff = (base_delay * exponential_factor.powf(retries as f64)) as u64;

        // Ensure the backoff does not exceed the maximum delay
        let final_backoff = backoff.min(max_delay);

        Duration::from_secs(final_backoff)
    }

    pub(crate) async fn query_state_update(
        &mut self,
        eth_from: u64,
        starknet_from: u64,
    ) -> Result<Vec<StateUpdate>, Error> {
        let highest_eth_block_number =
            self.http_provider.get_block_number().await.map_err(|e| Error::L1Connection(e.to_string()))?.as_u64();

        let filter = Filter::new().address(self.core_contract).event("LogStateUpdate(uint256,int256,uint256)");

        let mut from = eth_from;
        let mut to = eth_from + STATE_SEARCH_STEP;

        // TODO: Using more precise criteria to determine whether it is in a syncing state.
        if from > highest_eth_block_number {
            from = highest_eth_block_number;

            self.sync_status
                .lock()
                .and_then(|mut status| {
                    *status = SyncStatus::SYNCED;
                    Ok(())
                })
                .map_err(|e| Error::Other(e.to_string()))?;
        } else {
            self.sync_status
                .lock()
                .and_then(|mut status| {
                    *status = SyncStatus::SYNCING;
                    Ok(())
                })
                .map_err(|e| Error::Other(e.to_string()))?;
        }

        if to > highest_eth_block_number {
            to = highest_eth_block_number
        }

        loop {
            let filter = filter.clone().from_block(from).to_block(to);

            let updates: Result<Vec<StateUpdate>, Error> = self
                .get_logs_retry(&filter)
                .await?
                .iter()
                .map(|log| {
                    <LogStateUpdate as EthLogDecode>::decode_log(&(log.topics.clone(), log.data.to_vec()).into())
                        .map_err(|_| Error::L1EventDecode)
                        .and_then(|log_state_update| {
                            Ok(StateUpdate {
                                eth_origin: EthOrigin {
                                    block_hash: log.block_hash.ok_or(Error::L1EventDecode)?,
                                    block_number: log.block_number.ok_or(Error::L1EventDecode)?.as_u64(),
                                    _transaction_hash: log.transaction_hash.ok_or(Error::L1EventDecode)?,
                                    transaction_index: log.transaction_index.ok_or(Error::L1EventDecode)?.as_u64(),
                                },
                                update: log_state_update,
                            })
                        })
                })
                .filter(|res| {
                    if let Ok(state_update) = res {
                        if state_update.update.block_number.as_u64() < starknet_from {
                            return false;
                        }
                    }
                    true
                })
                .collect();

            if let Ok(remind_state_updates) = updates {
                if !remind_state_updates.is_empty() {
                    return Ok(remind_state_updates);
                }
            }

            from += LOG_SEARCH_STEP;
            to += LOG_SEARCH_STEP;
        }
    }

    pub async fn query_state_transition_fact(
        &mut self,
        eth_from: u64,
        tx_index: u64,
    ) -> Result<LogStateTransitionFact, Error> {
        let filter = Filter::new()
            .address(self.core_contract)
            .event("LogStateTransitionFact(bytes32)")
            .from_block(eth_from)
            .to_block(eth_from);

        self.get_logs_retry(&filter)
            .await?
            .iter()
            .find(|log| {
                if let Some(index) = log.transaction_index {
                    if index.as_u64() == tx_index {
                        return true;
                    }
                }
                false
            })
            .ok_or(Error::L1StateError(format!(
                "can't find starknet state transition fact from block:{}, tx:{}",
                eth_from, tx_index
            )))
            .and_then(|log| {
                <LogStateTransitionFact as EthLogDecode>::decode_log(&(log.topics.clone(), log.data.to_vec()).into())
                    .map_err(|_| Error::L1EventDecode)
            })
    }

    pub async fn query_memory_pages_hashes(
        &mut self,
        eth_from: u64,
        state_transition_fact: LogStateTransitionFact,
    ) -> Result<LogMemoryPagesHashes, Error> {
        let filter = Filter::new().address(self.verifier_contract).event("LogMemoryPagesHashes(bytes32,bytes32[])");

        let mut from = eth_from.saturating_sub(LOG_SEARCH_STEP);
        let mut to = eth_from;

        loop {
            if to == 0 {
                return Err(Error::Other(format!("find fact {:#?} failed", state_transition_fact)));
            }
            let filter = filter.clone().from_block(from).to_block(to);

            let res = self
                .get_logs_retry(&filter)
                .await?
                .iter()
                .find_map(|log| {
                    match <LogMemoryPagesHashes as EthLogDecode>::decode_log(
                        &(log.topics.clone(), log.data.to_vec()).into(),
                    ) {
                        Ok(pages_hashes) => {
                            if pages_hashes.fact.eq(&state_transition_fact.fact) {
                                return Some(pages_hashes);
                            }
                            None
                        }
                        Err(_) => None,
                    }
                })
                .ok_or(Error::L1StateError("memory pages not found".to_string()));

            if let Ok(pages_hashes) = res {
                return Ok(pages_hashes);
            }

            from = from.saturating_sub(LOG_SEARCH_STEP);
            to = to.saturating_sub(LOG_SEARCH_STEP);
        }
    }

    pub async fn query_memory_page_fact_continuous_logs(
        &mut self,
        eth_from: u64,
        pages_hashes: &mut Vec<U256>,
    ) -> Result<Vec<LogMemoryPageFactContinuousWithTxHash>, Error> {
        let filter = Filter::new()
            .address(self.memory_page_contract)
            .event("LogMemoryPageFactContinuous(bytes32,uint256,uint256)");

        let mut from = eth_from.saturating_sub(LOG_SEARCH_STEP);
        let mut to = eth_from;

        let mut match_pages_hashes = Vec::new();

        loop {
            if to == 0 {
                return Err(Error::Other(format!("find fact failed")));
            }
            let filter = filter.clone().from_block(from).to_block(to);

            let logs = self.get_logs_retry(&filter).await?;
            let mut memory_pages_hashes = Vec::new();

            for l in logs.iter() {
                let raw_log = RawLog::from(l.clone());
                let log_pages_fact_continuous = <LogMemoryPageFactContinuous as EthLogDecode>::decode_log(&raw_log)
                    .map_err(|_| Error::L1EventDecode)?;

                let pages_hashes_len = pages_hashes.len();
                pages_hashes.retain(|&elem| elem != log_pages_fact_continuous.memory_hash);
                if pages_hashes_len != pages_hashes.len() {
                    memory_pages_hashes.push(LogMemoryPageFactContinuousWithTxHash {
                        log_memory_page_fact_continuous: log_pages_fact_continuous,
                        tx_hash: l.transaction_hash.ok_or(Error::L1EventDecode)?,
                    })
                }
            }
            match_pages_hashes.push(memory_pages_hashes);

            if pages_hashes.is_empty() {
                break;
            }

            from = from.saturating_sub(LOG_SEARCH_STEP);
            to = to.saturating_sub(LOG_SEARCH_STEP);
        }

        match_pages_hashes.reverse();

        Ok(match_pages_hashes.into_iter().flatten().collect())
    }

    pub async fn query_and_decode_transaction(&mut self, hash: H256) -> Result<Vec<U256>, Error> {
        let tx = self
            .http_provider
            .get_transaction(hash)
            .await
            .map_err(|e| Error::L1Connection(e.to_string()))?
            .ok_or(Error::Other("query transaction by hash get none".to_string()))?;

        let abi = BaseContract::from(
            parse_abi(&["function registerContinuousMemoryPage(uint256 startAddr,uint256[] values,uint256 z,uint256 \
                         alpha,uint256 prime)"])
            .unwrap(),
        );

        let (_, data, _, _, _): (U256, Vec<U256>, U256, U256, U256) =
            abi.decode("registerContinuousMemoryPage", tx.input.as_ref()).unwrap();
        Ok(data)
    }

    pub fn decode_state_diff<B, C>(
        &self,
        l1_block_number: u64,
        starknet_block_number: u64,
        data: Vec<U256>,
        client: Arc<C>,
    ) -> Result<StateDiff, Error>
    where
        B: BlockT,
        C: ProvideRuntimeApi<B> + HeaderBackend<B>,
        C::Api: StarknetRuntimeApi<B>,
    {
        if l1_block_number < self.v011_diff_format_height {
            parser::decode_pre_011_diff(&data, false)
        } else {
            let block_hash = client
                .block_hash_from_id(&BlockId::Number((starknet_block_number as u32).saturating_sub(1).into()))
                .map_err(|_| Error::UnknownBlock)?
                .unwrap_or_default();
            parser::decode_011_diff(&data, block_hash, client)
        }
    }

    pub async fn query_state_diff<B, C>(
        &mut self,
        state_update: &StateUpdate,
        client: Arc<C>,
    ) -> Result<FetchState, Error>
    where
        B: BlockT,
        C: ProvideRuntimeApi<B> + HeaderBackend<B>,
        C::Api: StarknetRuntimeApi<B>,
    {
        debug!(target: LOG_TARGET,"~~ query state diff for starknet block {:#?}", state_update.update.block_number);

        let fact = self
            .query_state_transition_fact(
                state_update.eth_origin.block_number,
                state_update.eth_origin.transaction_index,
            )
            .await?;

        let pages_hashes = self.query_memory_pages_hashes(state_update.eth_origin.block_number, fact).await?;

        let mut pages_hashes =
            pages_hashes.pages_hashes.iter().map(|data| U256::from_big_endian(data)).collect::<Vec<_>>();

        let continuous_logs_with_tx_hash = self
            .query_memory_page_fact_continuous_logs(state_update.eth_origin.block_number, &mut pages_hashes)
            .await?;

        let mut tx_input_data = Vec::new();

        for log in &continuous_logs_with_tx_hash[1..] {
            debug!(target: LOG_TARGET,"~~ decode state diff from tx: {:#?}", log.tx_hash);

            let mut data = self.query_and_decode_transaction(log.tx_hash).await?;
            tx_input_data.append(&mut data)
        }

        let state_diff = self.decode_state_diff(
            state_update.eth_origin.block_number,
            state_update.update.block_number.as_u64(),
            tx_input_data,
            client,
        )?;

        Ok(FetchState {
            l1_l2_block_mapping: L1L2BlockMapping {
                l1_block_hash: state_update.eth_origin.block_hash,
                l1_block_number: state_update.eth_origin.block_number,
                l2_block_hash: state_update.update.block_hash,
                l2_block_number: state_update.update.block_number.as_u64(),
            },
            post_state_root: state_update.update.global_root,
            state_diff,
        })
    }
}

#[async_trait]
impl<P: JsonRpcClient + Clone> StateFetcher for EthereumStateFetcher<P> {
    async fn state_diff<B, C>(&mut self, l1_from: u64, l2_start: u64, client: Arc<C>) -> Result<Vec<FetchState>, Error>
    where
        B: BlockT,
        C: ProvideRuntimeApi<B> + HeaderBackend<B>,
        C::Api: StarknetRuntimeApi<B>,
    {
        debug!(target: LOG_TARGET, "state_diff {} {}", l1_from, l2_start);
        println!("state_diff {} {}", l1_from, l2_start);

        let state_updates = self.query_state_update(l1_from, l2_start).await?;

        let tasks = state_updates.iter().map(|updates| {
            let client_clone = client.clone();
            let mut fetcher = self.clone();
            async move { fetcher.query_state_diff(updates, client_clone).await }
        });

        let fetched_states = futures::future::join_all(tasks).await;

        let mut states_res = Vec::new();
        for fetched_state in fetched_states {
            match fetched_state {
                Ok(state) => states_res.push(state),
                Err(e) => return Err(e),
            }
        }

        Ok(states_res)
    }
}
