mod ethereum;
mod sync;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use async_trait::async_trait;
use ethers::types::U256;
use futures::prelude::*;
use futures::channel::mpsc;
use mc_db::L1L2BlockMapping;
use sc_client_api::backend::Backend;
use sp_blockchain::HeaderBackend;
use sp_core::H256;
use sp_runtime::generic::Header as GenericHeader;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};
use sync::StateWriter;

type EncodeStateDiff = Vec<U256>;

#[derive(Debug, Clone)]
pub struct FetchState {
    pub block_info: L1L2BlockMapping,
    pub encode_state_diff: EncodeStateDiff,
}

// BaseLayer
#[async_trait]
pub trait StateFetcher {
    async fn fetch_state_diff(&self, from_l1_block: u64, l2_start_block: u64) -> Result<Vec<FetchState>, Error>;
}

pub async fn run<B, C, BE>(
    state_fetcher: Box<dyn StateFetcher>,
    madara_backend: Arc<mc_db::Backend<B>>,
    substrate_client: Arc<C>,
    substrate_backend: Arc<BE>,
) -> Result<impl Future<Output = ()> + Send, Error>
where
    B: BlockT<Hash = H256, Header = GenericHeader<u32, BlakeTwo256>>,
    C: HeaderBackend<B>,
    BE: Backend<B>,
{
    let (tx, rx) = mpsc::unbounded::<FetchState>();
    let state_writer = StateWriter::new(substrate_client, substrate_backend, madara_backend);

    let fetcher_task = async {};

    let state_write_task = async {};

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
