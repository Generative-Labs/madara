mod ethereum;
mod sync;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use async_trait::async_trait;
use ethers::types::U256;
use futures::channel::mpsc;
use futures::prelude::*;
use mc_db::L1L2BlockMapping;
use sc_client_api::backend::Backend;
use sp_blockchain::HeaderBackend;
use sp_core::H256;
use sp_runtime::generic::Header as GenericHeader;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};
use sync::StateWriter;

use crate::sync::SyncStateDiff;

type EncodeStateDiff = Vec<U256>;

#[derive(Debug, Clone)]
pub struct FetchState {
    pub block_info: L1L2BlockMapping,
    pub encode_state_diff: EncodeStateDiff,
}

#[async_trait]
pub trait StateFetcher {
    async fn fetch_state_diff(&self, from_l1_block: u64, l2_start_block: u64) -> Result<Vec<FetchState>, Error>;
}

pub async fn run<B, C, BE, SF>(
    state_fetcher: Arc<SF>,
    madara_backend: Arc<mc_db::Backend<B>>,
    substrate_client: Arc<C>,
    substrate_backend: Arc<BE>,
) -> Result<impl Future<Output = ()> + Send, Error>
where
    B: BlockT<Hash = H256, Header = GenericHeader<u32, BlakeTwo256>>,
    C: HeaderBackend<B> + 'static,
    BE: Backend<B> + 'static,
    SF: StateFetcher + Send + Sync + 'static,
{
    let (mut tx, mut rx) = mpsc::unbounded::<FetchState>();

    let state_writer = StateWriter::new(substrate_client, substrate_backend, madara_backend);
    let state_writer = Arc::new(state_writer);
    let state_fetcher_clone = state_fetcher.clone();

    let fetcher_task = async move {
        loop {
            if let Ok(fs) = state_fetcher_clone.fetch_state_diff(10, 11).await {
                for s in fs.iter() {
                    let _ = tx.send(s.clone());
                }
            }
            // time.sleep() need sleep ??
        }
    };

    let state_write_task = async move {
        loop {
            if let Some(s) = rx.next().await {
                println!("{:#?}", s.block_info);
                let _ = state_writer.apply_state_diff(0, SyncStateDiff::default());
            }
        }
    };

    let task = future::join(fetcher_task, state_write_task).map(|_| ()).boxed();

    Ok(task)
}

#[derive(Debug)]
pub enum Error {
    AlreadyInChain,
    UnknownBlock,
    ConstructTransaction(String),
    CommitStorage(String),
    L1Connection(String),
    L1EventDecode,
    Other(String),
}
