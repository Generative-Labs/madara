use std::marker::PhantomData;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use ethers::types::I256;
use mp_transactions::Transaction;
use sc_client_api::backend::Backend;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use starknet_api::block::{BlockHash, BlockNumber};
use starknet_api::state::StateDiff;

#[cfg(test)]
pub mod tests;

#[async_trait]
pub trait L1StateProvider {
    async fn latest_proved_block(&self) -> Result<(BlockNumber, BlockHash)>;

    async fn get_state_diffs(&self, l2_block_number: I256) -> Result<(BlockHash, StateDiff)>;

    async fn get_transaction(&self, l2_block_number: I256) -> Result<Vec<Transaction>>;
}

pub struct StateSyncWorker<B: BlockT, C, BE> {
    client: Arc<C>,
    substrate_backend: Arc<BE>,
    madara_backend: Arc<mc_db::Backend<B>>,
    l1_state_provider: Box<dyn L1StateProvider>,
    phantom_data: PhantomData<B>,
}

impl<B: BlockT, C, BE> StateSyncWorker<B, C, BE>
where
    C: HeaderBackend<B>,
    BE: Backend<B>,
{
    pub fn new(
        client: Arc<C>,
        substrate_backend: Arc<BE>,
        madara_backend: Arc<mc_db::Backend<B>>,
        l1_state_provider: Box<dyn L1StateProvider>,
    ) -> Self {
        Self { client, substrate_backend, l1_state_provider, madara_backend, phantom_data: PhantomData }
    }

    // Apply the state difference to the data layer.
    // Since the madara block is currently wrapped within a substrate block,
    // and the madara blockchain does not have an independent state root,
    // we temporarily use the highest Substrate block as the latest state.
    // Then, we apply the state difference to the state represented by the state root of this block.
    fn apply_state_diff(&mut self, state_diff: StateDiff) {
        // Backend::begin_state_operation, Backend::commit_operation. 
    }
}
