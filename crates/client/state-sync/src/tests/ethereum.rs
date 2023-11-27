use ethers::types::{Address, H256};

use crate::ethereum::EthereumStateFetcher;

#[tokio::test]
async fn test_get_state_update() {
    let contract_address = "0xc662c410c0ecf747543f5ba90660f6abebd9c8c4".parse::<Address>().unwrap();
    let verifier_address = "0x47312450B3Ac8b5b8e247a6bB6d523e7605bDb60".parse::<Address>().unwrap();
    let memory_page_address = "0xdc1534eeBF8CEEe76E31C98F5f5e0F9979476c87".parse::<Address>().unwrap();

    let eth_mainnet_url = "https://eth.llamarpc.com".to_string();

    let client =
        EthereumStateFetcher::new(eth_mainnet_url, contract_address, verifier_address, memory_page_address).unwrap();
    client.query_state_update(18623979, 18623979).await.unwrap();
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
    // let contract_address =
    // "0xde29d060D45901Fb19ED6C6e959EB22d8626708e".parse::<Address>().unwrap();
    // let verifier_address =
    // "0xb59D5F625b63fbb04134213A526AA3762555B853".parse::<Address>().unwrap();
    // let memory_page_address =
    // "0xdc1534eeBF8CEEe76E31C98F5f5e0F9979476c87".parse::<Address>().unwrap();

    // let eth_mainnet_url = "https://eth-goerli.g.alchemy.com/v2/nMMxqPTld6cj0DUO-4Qj2cg88Dd1MUhH".to_string();

    // let client =
    //     EthereumStateFetcher::new(eth_mainnet_url, contract_address, verifier_address,
    // memory_page_address).unwrap(); client.query_memory_pages_hashes(10000296,
    // 10000296).await;
}

#[tokio::test]
async fn get_memory_page_fact_continuous_logs() {
    let contract_address = "0xde29d060D45901Fb19ED6C6e959EB22d8626708e".parse::<Address>().unwrap();
    let verifier_address = "0xb59D5F625b63fbb04134213A526AA3762555B853".parse::<Address>().unwrap();
    let memory_page_address = "0xdc1534eeBF8CEEe76E31C98F5f5e0F9979476c87".parse::<Address>().unwrap();

    let eth_mainnet_url = "https://eth-goerli.g.alchemy.com/v2/nMMxqPTld6cj0DUO-4Qj2cg88Dd1MUhH".to_string();

    let client =
        EthereumStateFetcher::new(eth_mainnet_url, contract_address, verifier_address, memory_page_address).unwrap();

    // client.query_memory_page_fact_continuous_logs(10087516, 10087516).await;
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
