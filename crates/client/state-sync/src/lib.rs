mod ethereum;
mod parser;
mod sync;

#[cfg(test)]
mod tests;

use std::cmp::Ordering;
use std::sync::Arc;

use async_trait::async_trait;
use ethers::types::{H256, U256};
use futures::channel::mpsc;
use futures::prelude::*;
use log::error;
use mc_db::L1L2BlockMapping;
use pallet_starknet::runtime_api::StarknetRuntimeApi;
use sc_client_api::backend::Backend;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::generic::Header as GenericHeader;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};
use starknet_api::state::StateDiff;
use sync::StateWriter;

const LOG_TARGET: &'static str = "state-sync";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchState {
    pub l1_l2_block_mapping: L1L2BlockMapping,
    pub post_state_root: U256,
    pub state_diff: StateDiff,
}

impl PartialOrd for FetchState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.l1_l2_block_mapping.l2_block_number.cmp(&other.l1_l2_block_mapping.l2_block_number))
    }
}

impl Ord for FetchState {
    fn cmp(&self, other: &Self) -> Ordering {
        self.l1_l2_block_mapping.l2_block_number.cmp(&other.l1_l2_block_mapping.l2_block_number)
    }
}

#[async_trait]
pub trait StateFetcher {
    async fn state_diff<B, C>(&self, l1_from: u64, l2_start: u64, client: Arc<C>) -> Result<Vec<FetchState>, Error>
    where
        B: BlockT,
        C: ProvideRuntimeApi<B> + HeaderBackend<B>,
        C::Api: StarknetRuntimeApi<B>;
}

// TODO pass a config then create state_fetcher
pub async fn run<B, C, BE, SF>(
    state_fetcher: Arc<SF>,
    madara_backend: Arc<mc_db::Backend<B>>,
    substrate_client: Arc<C>,
    substrate_backend: Arc<BE>,
) -> Result<impl Future<Output = ()> + Send, Error>
where
    B: BlockT<Hash = H256, Header = GenericHeader<u32, BlakeTwo256>>,
    C: HeaderBackend<B> + ProvideRuntimeApi<B> + 'static,
    C::Api: StarknetRuntimeApi<B>,
    BE: Backend<B> + 'static,
    SF: StateFetcher + Send + Sync + 'static,
{
    let (mut tx, mut rx) = mpsc::unbounded::<Vec<FetchState>>();

    let state_writer = StateWriter::new(substrate_client.clone(), substrate_backend, madara_backend.clone());
    let state_writer = Arc::new(state_writer);
    let state_fetcher_clone = state_fetcher.clone();

    let madara_backend_clone = madara_backend.clone();
    let fetcher_task = async move {
        let mut eth_start_height = 0u64;
        let mut starknet_start_height = 0u64;

        let l1_l2_mapping = madara_backend_clone.clone().meta().last_l1_l2_mapping();

        match l1_l2_mapping {
            Ok(mapping) => {
                eth_start_height = mapping.l1_block_number + 1;
                starknet_start_height = mapping.l2_block_number + 1;
            }
            Err(_) => {}
        }

        loop {
            if let Ok(mut fetched_states) =
                state_fetcher_clone.state_diff(eth_start_height, starknet_start_height, substrate_client.clone()).await
            {
                fetched_states.sort();

                if let Some(last) = fetched_states.last() {
                    eth_start_height = last.l1_l2_block_mapping.l1_block_number + 1;
                    starknet_start_height = last.l1_l2_block_mapping.l2_block_number + 1;
                }

                let _ = tx.send(fetched_states);
            }
            // TODO time.sleep() need sleep ??
        }
    };

    let state_write_task = async move {
        loop {
            if let Some(fetched_states) = rx.next().await {
                for state in fetched_states.iter() {
                    let _ = state_writer.apply_state_diff(state.l1_l2_block_mapping.l2_block_number, &state.state_diff);
                }

                if let Some(last) = fetched_states.last() {
                    if let Err(e) = madara_backend.meta().write_last_l1_l2_mapping(&last.l1_l2_block_mapping) {
                        error!(target: LOG_TARGET, "write to madara backend has error {}", e);
                        break;
                    }
                }
            }
        }
    };

    let task =
        future::ready(()).then(move |_| future::select(Box::pin(fetcher_task), Box::pin(state_write_task))).map(|_| ());

    Ok(task)
}

#[derive(Debug, Clone)]
pub enum Error {
    AlreadyInChain,
    UnknownBlock,
    ConstructTransaction(String),
    CommitStorage(String),
    L1Connection(String),
    L1EventDecode,
    L1StateError(String),
    TypeError(String),
    Other(String),
}
