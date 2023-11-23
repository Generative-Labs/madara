mod ethereum;
mod ethereum_1;
mod l1;
mod retry;
mod sync;

#[cfg(test)]
mod tests;

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use ethers::types::U256;
use futures::Future;
use mc_db::L1L2BlockMapping;
use sp_runtime::traits::Block as BlockT;

type EncodeStateDiff = Vec<U256>;

pub(crate) struct FetchState {
    pub block_info: L1L2BlockMapping,
    pub encode_state_diff: EncodeStateDiff,
}

// BaseLayer
#[async_trait]
pub(crate) trait StateFetcher {
    async fn fetch_state_diff(
        &self,
        from_l1_block: u64,
        to_l1_block: u64,
        l2_start_block: u64,
    ) -> Result<Vec<FetchState>, Error>;
}

pub struct SyncWorker<B: BlockT> {
    state_fetcher: Box<dyn StateFetcher>,
    madara_backend: Arc<mc_db::Backend<B>>,
}

impl<B: BlockT> Future for SyncWorker<B> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            if let Ok(_last_l1_l2_mapping) = self.madara_backend.meta().last_l1_l2_mapping() {
                let _ = Pin::new(&mut self.state_fetcher.fetch_state_diff(10, 111, 1)).poll(cx);
            }
        }
        Poll::Ready(())
    }
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
