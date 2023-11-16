use std::{time::Duration, num::NonZeroU64};
use primitive_types::H160;
use mp_hashers::HasherT;
// use tokio::sync::mpsc;

use crate::ethereum::EthereumApi;

use super::retry::Retry;


#[derive(Clone)]
pub struct L1SyncContext<EthereumClient> {
    pub ethereum: EthereumClient,
    /// The Starknet core contract address on Ethereum
    pub core_address: H160,
    pub poll_interval: Duration,
}


pub async fn sync_from_l1_loop<T>(
    context: L1SyncContext<T>,
) -> anyhow::Result<()>
where
    T: EthereumApi + Clone,
{
    let L1SyncContext {
        ethereum,
        core_address,
        poll_interval,
    } = context;

	loop {
        let state_update = Retry::exponential(
            || async { ethereum.get_starknet_state(&core_address).await },
            NonZeroU64::new(1).unwrap(),
        )
        .factor(NonZeroU64::new(2).unwrap())
        .max_delay(poll_interval / 2)
        .when(|_| true)
        .await?;

        tokio::time::sleep(poll_interval).await;
    }
}
