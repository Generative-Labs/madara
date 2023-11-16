use std::marker::PhantomData;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use blockifier::state::cached_state::CommitmentStateDiff;
use ethers::types::I256;
use mp_transactions::Transaction;
use sc_client_api::backend::{Backend, BlockImportOperation};
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use starknet_api::block::{BlockHash, BlockNumber};
use starknet_api::state::StateDiff;
use starknet_api::api_core::{ClassHash, CompiledClassHash, ContractAddress, Nonce, PatriciaKey};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey as StarknetStorageKey;
use mp_storage::{SN_COMPILED_CLASS_HASH_PREFIX, SN_CONTRACT_CLASS_HASH_PREFIX, SN_NONCE_PREFIX, SN_STORAGE_PREFIX};
use indexmap::IndexMap;
use frame_support::{Identity, StorageHasher};
use sp_state_machine::{ StorageKey, StorageValue};
use sp_core::Encode;

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
    ) -> Self {
        Self { client, substrate_backend, phantom_data: PhantomData }
    }

    // Apply the state difference to the data layer.
    // Since the madara block is currently wrapped within a substrate block,
    // and the madara blockchain does not have an independent state root,
    // we temporarily use the highest Substrate block as the latest state.
    // Then, we apply the state difference to the state represented by the state root of this block.
    fn apply_state_diff(&mut self, commitment_state_diff: CommitmentStateDiff) {
        // Backend::begin_state_operation, Backend::commit_operation.
        let block_info = self.client.info();
        //let block_import_operation = BlockImportOperation::<B>::default();
        // self.substrate_backend.begin_state_operation();
        let mut operation = self.substrate_backend.begin_operation().unwrap();
        let storage_changes: InnerStorageChangeSet = commitment_state_diff.into();
        operation.update_storage(storage_changes.changes, storage_changes.child_changes).unwrap();
        self.substrate_backend.begin_state_operation(&mut operation, block_info.best_hash).unwrap();
        self.substrate_backend.commit_operation(operation).unwrap();
    }
}

// InnerStorageChangeSet just used for test
#[derive(Debug, PartialEq, Eq)]
struct InnerStorageChangeSet {
    changes: Vec<(StorageKey, Option<StorageValue>)>,
    child_changes: Vec<(StorageKey, Vec<(StorageKey, Option<StorageValue>)>)>,
}

impl InnerStorageChangeSet {
    pub fn iter(&self) -> impl Iterator<Item = (Option<&StorageKey>, &StorageKey, Option<&StorageValue>)> + '_ {
        let top = self.changes.iter().map(|(k, v)| (None, k, v.as_ref()));
        let children = self
            .child_changes
            .iter()
            .flat_map(|(sk, changes)| changes.iter().map(move |(k, v)| (Some(sk), k, v.as_ref())));
        top.chain(children)
    }
}

impl Into<CommitmentStateDiff> for InnerStorageChangeSet {
    fn into(self) -> CommitmentStateDiff {
        let mut commitment_state_diff = CommitmentStateDiff {
            address_to_class_hash: Default::default(),
            address_to_nonce: Default::default(),
            storage_updates: Default::default(),
            class_hash_to_compiled_class_hash: Default::default(),
        };

        for (_prefix, full_storage_key, change) in self.iter() {
            // The storages we are interested in all have prefix of length 32 bytes.
            // The pallet identifier takes 16 bytes, the storage one 16 bytes.
            // So if a storage key is smaller than 32 bytes,
            // the program will panic when we index it to get it's prefix
            if full_storage_key.len() < 32 {
                continue;
            }
            let prefix = &full_storage_key[..32];

            // All the `try_into` are safe to `unwrap` because we know what the storage contains
            // and therefore what size it is
            if prefix == *SN_NONCE_PREFIX {
                let contract_address =
                    ContractAddress(PatriciaKey(StarkFelt(full_storage_key[32..].try_into().unwrap())));
                // `change` is safe to unwrap as `Nonces` storage is `ValueQuery`
                let nonce = Nonce(StarkFelt(change.unwrap().clone().try_into().unwrap()));
                commitment_state_diff.address_to_nonce.insert(contract_address, nonce);
            } else if prefix == *SN_STORAGE_PREFIX {
                let contract_address =
                    ContractAddress(PatriciaKey(StarkFelt(full_storage_key[32..64].try_into().unwrap())));
                let storage_key =
                    StarknetStorageKey(PatriciaKey(StarkFelt(full_storage_key[64..].try_into().unwrap())));
                // `change` is safe to unwrap as `StorageView` storage is `ValueQuery`
                let value = StarkFelt(change.unwrap().clone().try_into().unwrap());

                match commitment_state_diff.storage_updates.get_mut(&contract_address) {
                    Some(contract_storage) => {
                        contract_storage.insert(storage_key, value);
                    }
                    None => {
                        let mut contract_storage: IndexMap<_, _, _> = Default::default();
                        contract_storage.insert(storage_key, value);

                        commitment_state_diff.storage_updates.insert(contract_address, contract_storage);
                    }
                }
            } else if prefix == *SN_CONTRACT_CLASS_HASH_PREFIX {
                let contract_address =
                    ContractAddress(PatriciaKey(StarkFelt(full_storage_key[32..].try_into().unwrap())));
                // `change` is safe to unwrap as `ContractClassHashes` storage is `ValueQuery`
                let class_hash = ClassHash(StarkFelt(change.unwrap().clone().try_into().unwrap()));

                commitment_state_diff.address_to_class_hash.insert(contract_address, class_hash);
            } else if prefix == *SN_COMPILED_CLASS_HASH_PREFIX {
                let class_hash = ClassHash(StarkFelt(full_storage_key[32..].try_into().unwrap()));
                // In the current state of starknet protocol, a compiled class hash can not be erased, so we should
                // never see `change` being `None`. But there have been an "erase contract class" mechanism live on
                // the network during the Regenesis migration. Better safe than sorry.
                let compiled_class_hash = CompiledClassHash(
                    change.map(|data| StarkFelt(data.clone().try_into().unwrap())).unwrap_or_default(),
                );

                commitment_state_diff.class_hash_to_compiled_class_hash.insert(class_hash, compiled_class_hash);
            }
        }

        commitment_state_diff
    }
}

impl From<CommitmentStateDiff> for InnerStorageChangeSet {
    fn from(commitment_state_diff: CommitmentStateDiff) -> Self {
        let mut changes: Vec<(StorageKey, Option<StorageValue>)> = Vec::new();
        // now starknet not use child changes.
        let mut _child_changes: Vec<(StorageKey, Vec<(StorageKey, Option<StorageValue>)>)> = Vec::new();

        for (address, class_hash) in commitment_state_diff.address_to_class_hash.iter() {
            let storage_key = storage_key_build(SN_CONTRACT_CLASS_HASH_PREFIX.clone(), &address.encode());
            let storage_value = class_hash.encode();
            changes.push((storage_key, Some(storage_value)));
        }

        for (address, nonce) in commitment_state_diff.address_to_nonce.iter() {
            let storage_key = storage_key_build(SN_NONCE_PREFIX.clone(), &address.encode());
            let storage_value = nonce.encode();
            changes.push((storage_key, Some(storage_value)));
        }

        for (address, storages) in commitment_state_diff.storage_updates.iter() {
            for (sk, value) in storages.iter(){
                let storage_key = storage_key_build(SN_STORAGE_PREFIX.clone(),&[address.encode(), sk.encode()].concat());
                let storage_value = value.encode();
                changes.push((storage_key, Some(storage_value)));
            }
        }

        for (address, compiled_class_hash) in commitment_state_diff.class_hash_to_compiled_class_hash.iter() {
            let storage_key = storage_key_build(SN_COMPILED_CLASS_HASH_PREFIX.clone(), &address.encode());
            let storage_value = compiled_class_hash.encode();
            changes.push((storage_key, Some(storage_value)));
        }

        InnerStorageChangeSet { changes, child_changes: _child_changes }
    }
}

pub fn storage_key_build(prefix: Vec<u8>, key: &[u8]) -> Vec<u8> {
    [prefix, Identity::hash(key)].concat()
}
