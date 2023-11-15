use std::marker::PhantomData;
use std::sync::Arc;

use ethers::types::U256;
use mp_block::L1StarknetHead;
// Substrate
use scale_codec::{Decode, Encode};
use sp_database::Database;
use sp_runtime::traits::Block as BlockT;
use uuid::Uuid;

use crate::DbHash;

// The fact db stores DA facts that need to be written to L1
pub struct DaDb<B: BlockT> {
    pub(crate) db: Arc<dyn Database<DbHash>>,
    pub(crate) _marker: PhantomData<B>,
}

// TODO: purge old cairo job keys
impl<B: BlockT> DaDb<B> {
    pub fn state_diff(&self, block_hash: &B::Hash) -> Result<Vec<U256>, String> {
        match self.db.get(crate::columns::DA, &block_hash.encode()) {
            Some(raw) => Ok(Vec::<U256>::decode(&mut &raw[..]).map_err(|e| format!("{:?}", e))?),
            None => Ok(Vec::new()),
        }
    }

    pub fn store_state_diff(&self, block_hash: &B::Hash, diffs: Vec<U256>) -> Result<(), String> {
        let mut transaction = sp_database::Transaction::new();

        transaction.set(crate::columns::DA, &block_hash.encode(), &diffs.encode());

        self.db.commit(transaction).map_err(|e| format!("{:?}", e))?;

        Ok(())
    }

    pub fn cairo_job(&self, block_hash: &B::Hash) -> Result<Uuid, String> {
        match self.db.get(crate::columns::DA, &block_hash.encode()) {
            Some(raw) => Ok(Uuid::from_slice(&raw[..]).map_err(|e| format!("{:?}", e))?),
            None => Err(String::from("can't locate cairo job")),
        }
    }

    pub fn update_cairo_job(&self, block_hash: &B::Hash, job_id: Uuid) -> Result<(), String> {
        let mut transaction = sp_database::Transaction::new();

        transaction.set(crate::columns::DA, &block_hash.encode(), &job_id.into_bytes());

        self.db.commit(transaction).map_err(|e| format!("{:?}", e))?;

        Ok(())
    }

    pub fn last_proved_block(&self) -> Result<B::Hash, String> {
        match self.db.get(crate::columns::DA, crate::static_keys::LAST_PROVED_BLOCK) {
            Some(raw) => Ok(B::Hash::decode(&mut &raw[..]).map_err(|e| format!("{:?}", e))?),
            None => Err(String::from("can't locate last proved block")),
        }
    }

    pub fn update_last_proved_block(&self, block_hash: &B::Hash) -> Result<(), String> {
        let mut transaction = sp_database::Transaction::new();

        transaction.set(crate::columns::DA, crate::static_keys::LAST_PROVED_BLOCK, &block_hash.encode());

        self.db.commit(transaction).map_err(|e| format!("{:?}", e))?;

        Ok(())
    }

    /// Store the starknet block header sync from l1
    pub fn write_l1_starknet_block_header(&self, l1_header: &L1StarknetHead) -> Result<(), String> {
        let mut transaction = sp_database::Transaction::new();

        transaction.set(crate::columns::L1_HEADER, &l1_header.block_number.encode(), &l1_header.encode());
        transaction.set(crate::columns::L1_HEADER, &l1_header.block_hash.encode(), &l1_header.block_number.encode());

        self.db.commit(transaction).map_err(|e| format!("{:?}", e))?;

        Ok(())
    }

    pub fn l1_starknet_block_header_from_block_hash(&self, block_hash: B::Hash) -> Result<L1StarknetHead, String> {
        match self.db.get(crate::columns::L1_HEADER, &block_hash.encode()) {
            Some(raw) => self.l1_starknet_block_header_from_block_number(
                u64::decode(&mut &raw[..]).map_err(|e| format!("decode error {}", e.to_string()))?,
            ),
            None => Err(format!("no found starknet block header from block hash: {}", block_hash)),
        }
    }

    pub fn l1_starknet_block_header_from_block_number(&self, block_number: u64) -> Result<L1StarknetHead, String> {
        match self.db.get(crate::columns::L1_HEADER, &block_number.encode()) {
            Some(raw) => L1StarknetHead::decode(&mut &raw[..]).map_err(|e| e.to_string()),
            None => Err(format!("no found starknet block header from block number {}", block_number)),
        }
    }
}
