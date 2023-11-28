use std::sync::Arc;

use ethers::types::Address;

use crate::ethereum::EthereumStateFetcher;
use crate::tests::sync::create_test_client;
use crate::StateFetcher;

#[tokio::test]
async fn test_fetch_and_decode_state_diff() {
    let contract_address = "0xde29d060D45901Fb19ED6C6e959EB22d8626708e".parse::<Address>().unwrap();
    let verifier_address = "0xb59D5F625b63fbb04134213A526AA3762555B853".parse::<Address>().unwrap();
    let memory_page_address = "0xdc1534eeBF8CEEe76E31C98F5f5e0F9979476c87".parse::<Address>().unwrap();

    let url = String::from("https://eth-goerli.g.alchemy.com/v2/nMMxqPTld6cj0DUO-4Qj2cg88Dd1MUhH");
    let fetcher = EthereumStateFetcher::new(url, contract_address, verifier_address, memory_page_address).unwrap();

    let l1_from = 9064758;
    let l2_start = 809819;

    let (madara_client, _) = create_test_client();

    let result = fetcher.state_diff(l1_from, l2_start, Arc::new(madara_client)).await.unwrap();
    assert!(!result.is_empty());
}
