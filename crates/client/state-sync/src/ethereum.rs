use async_trait::async_trait;
use ethers::abi::RawLog;
use ethers::contract::{BaseContract, EthEvent, EthLogDecode};
use ethers::core::abi::parse_abi;
use ethers::providers::{Http, Middleware, Provider};
use ethers::types::{Address, Bytes, Filter, Log, Topic, H160, H256, I256, U256};
use starknet_api::block::{BlockHash, BlockNumber};
use starknet_api::hash::{StarkFelt, StarkHash};

use crate::{Error, FetchState, StateFetcher};

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
    pub state_transition_fact: [u8; 32],
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

pub struct EthereumStateFetcher {
    http_provider: Provider<Http>,

    core_contract: Address,

    verifier_contract: Address,

    memory_page_contract: Address,
}

impl EthereumStateFetcher {
    pub fn new(
        url: String,
        core_contract: Address,
        verifier_contract: Address,
        memory_page_contract: Address,
    ) -> Result<Self, Error> {
        let provider = Provider::<Http>::try_from(url).map_err(|e| Error::L1Connection(e.to_string()))?;
        Ok(Self { http_provider: provider, core_contract, verifier_contract, memory_page_contract })
    }

    pub async fn query_state_update(
        &self,
        from_eth_height: u64,
        to_eth_height: u64,
    ) -> Result<Vec<LogStateUpdate>, Error> {
        let filter = Filter::new()
            .address(self.core_contract)
            .event("LogStateUpdate(uint256,int256,uint256)")
            .from_block(from_eth_height)
            .to_block(to_eth_height);

        let logs = self.http_provider.get_logs(&filter).await.map_err(|e| Error::L1Connection(e.to_string()))?;

        let mut starknet_state_updates = Vec::new();
        for l in logs.iter() {
            let raw_log = RawLog::from(l.clone());
            starknet_state_updates
                .push(<LogStateUpdate as EthLogDecode>::decode_log(&raw_log).map_err(|_| Error::L1EventDecode)?);
        }

        Ok(starknet_state_updates)
    }

    pub async fn query_state_transition_fact(
        &self,
        from_eth_height: u64,
        to_eth_height: u64,
    ) -> Result<Vec<LogStateTransitionFact>, Error> {
        let filter = Filter::new()
            .address(self.core_contract)
            .event("LogStateTransitionFact(bytes32)")
            .from_block(from_eth_height)
            .to_block(to_eth_height);

        let logs = self.http_provider.get_logs(&filter).await.map_err(|e| Error::L1Connection(e.to_string()))?;

        let mut starknet_state_transaction_facts = Vec::new();
        for l in logs.iter() {
            let raw_log = RawLog::from(l.clone());
            starknet_state_transaction_facts.push(
                <LogStateTransitionFact as EthLogDecode>::decode_log(&raw_log).map_err(|_| Error::L1EventDecode)?,
            );
        }

        Ok(starknet_state_transaction_facts)
    }

    pub async fn query_memory_pages_hashes(
        &self,
        from_eth_height: u64,
        to_eth_height: u64,
    ) -> Result<Vec<LogMemoryPagesHashes>, Error> {
        let filter = Filter::new()
            .address(self.verifier_contract)
            .event("LogMemoryPagesHashes(bytes32,bytes32[])")
            .from_block(from_eth_height)
            .to_block(to_eth_height);

        let logs = self.http_provider.get_logs(&filter).await.map_err(|e| Error::L1Connection(e.to_string()))?;

        let mut memory_pages_hashes = Vec::new();

        for l in logs.iter() {
            let raw_log = RawLog::from(l.clone());
            memory_pages_hashes
                .push(<LogMemoryPagesHashes as EthLogDecode>::decode_log(&raw_log).map_err(|_| Error::L1EventDecode)?);
        }

        Ok(memory_pages_hashes)
    }

    pub async fn query_memory_page_fact_continuous_logs(
        &self,
        from_eth_height: u64,
        to_eth_height: u64,
    ) -> Result<Vec<LogMemoryPageFactContinuous>, Error> {
        let filter = Filter::new()
            .address(self.memory_page_contract)
            .event("LogMemoryPageFactContinuous(bytes32,uint256,uint256)")
            .from_block(from_eth_height)
            .to_block(to_eth_height);

        let logs = self.http_provider.get_logs(&filter).await.map_err(|e| Error::L1Connection(e.to_string()))?;

        let mut memory_pages_hashes = Vec::new();

        for l in logs.iter() {
            let raw_log = RawLog::from(l.clone());
            memory_pages_hashes.push(
                <LogMemoryPageFactContinuous as EthLogDecode>::decode_log(&raw_log)
                    .map_err(|_| Error::L1EventDecode)?,
            );
            println!("{:#?}", l.transaction_hash);
        }

        Ok(memory_pages_hashes)
    }

    pub async fn query_and_decode_transaction(&self, hash: H256) -> Result<Vec<U256>, Error> {
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
}

#[async_trait]
impl StateFetcher for EthereumStateFetcher {
    async fn fetch_state_diff(&self, from_l1_block: u64, l2_start_block: u64) -> Result<Vec<FetchState>, Error> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn test_get_state_update() {
    let contract_address = "0xc662c410c0ecf747543f5ba90660f6abebd9c8c4".parse::<Address>().unwrap();
    let verifier_address = "0x47312450B3Ac8b5b8e247a6bB6d523e7605bDb60".parse::<Address>().unwrap();
    let memory_page_address = "0xdc1534eeBF8CEEe76E31C98F5f5e0F9979476c87".parse::<Address>().unwrap();

    let eth_mainnet_url = "https://eth.llamarpc.com".to_string();

    let client =
        EthereumStateFetcher::new(eth_mainnet_url, contract_address, verifier_address, memory_page_address).unwrap();
    client.query_state_update(18623979, 18623980).await.unwrap();
}

#[tokio::test]
async fn test_get_state_transition_fact() {
    let contract_address = "0xc662c410c0ecf747543f5ba90660f6abebd9c8c4".parse::<Address>().unwrap();
    let verifier_address = "0x47312450B3Ac8b5b8e247a6bB6d523e7605bDb60".parse::<Address>().unwrap();
    let memory_page_address = "0xdc1534eeBF8CEEe76E31C98F5f5e0F9979476c87".parse::<Address>().unwrap();

    let eth_mainnet_url = "https://eth.llamarpc.com".to_string();

    let client =
        EthereumStateFetcher::new(eth_mainnet_url, contract_address, verifier_address, memory_page_address).unwrap();
    client.query_state_transition_fact(18626167u64, 18626168u64).await.unwrap();
}

#[tokio::test]
async fn test_get_memory_pages_hashes() {
    let contract_address = "0xde29d060D45901Fb19ED6C6e959EB22d8626708e".parse::<Address>().unwrap();
    let verifier_address = "0xb59D5F625b63fbb04134213A526AA3762555B853".parse::<Address>().unwrap();
    let memory_page_address = "0xdc1534eeBF8CEEe76E31C98F5f5e0F9979476c87".parse::<Address>().unwrap();

    let eth_mainnet_url = "https://eth-goerli.g.alchemy.com/v2/nMMxqPTld6cj0DUO-4Qj2cg88Dd1MUhH".to_string();

    let client =
        EthereumStateFetcher::new(eth_mainnet_url, contract_address, verifier_address, memory_page_address).unwrap();
    client.query_memory_pages_hashes(10000296, 10000296).await;
}

#[tokio::test]
async fn get_memory_page_fact_continuous_logs() {
    let contract_address = "0xde29d060D45901Fb19ED6C6e959EB22d8626708e".parse::<Address>().unwrap();
    let verifier_address = "0xb59D5F625b63fbb04134213A526AA3762555B853".parse::<Address>().unwrap();
    let memory_page_address = "0xdc1534eeBF8CEEe76E31C98F5f5e0F9979476c87".parse::<Address>().unwrap();

    let eth_mainnet_url = "https://eth-goerli.g.alchemy.com/v2/nMMxqPTld6cj0DUO-4Qj2cg88Dd1MUhH".to_string();

    let client =
        EthereumStateFetcher::new(eth_mainnet_url, contract_address, verifier_address, memory_page_address).unwrap();

    client.query_memory_page_fact_continuous_logs(10087516, 10087516).await;
}

#[tokio::test]
async fn decode_transaction() {
    let contract_address = "0xde29d060D45901Fb19ED6C6e959EB22d8626708e".parse::<Address>().unwrap();
    let verifier_address = "0xb59D5F625b63fbb04134213A526AA3762555B853".parse::<Address>().unwrap();
    let memory_page_address = "0xdc1534eeBF8CEEe76E31C98F5f5e0F9979476c87".parse::<Address>().unwrap();

    let eth_mainnet_url = "https://eth-goerli.g.alchemy.com/v2/nMMxqPTld6cj0DUO-4Qj2cg88Dd1MUhH".to_string();

    let client =
        EthereumStateFetcher::new(eth_mainnet_url, contract_address, verifier_address, memory_page_address).unwrap();

    let tx_hash = "0x68a68fce176bb37aa3bbb6f19f68fb8d8d6401f1f8bec07456c99500a9740dca".parse::<H256>().unwrap();

    client.query_and_decode_transaction(tx_hash).await;
}
