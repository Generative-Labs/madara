use primitive_types::U256;
use starknet_api::state::{StateDiff, StorageKey, ContractClass};
use indexmap::IndexMap;
use starknet_api::api_core::{
	ClassHash, CompiledClassHash, ContractAddress, EntryPointSelector, GlobalRoot, Nonce,
	PatriciaKey,
};
use mp_felt::Felt252Wrapper;
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::deprecated_contract_class::ContractClass as DeprecatedContractClass;
use crate::Error;

const NUM_STORAGE_UPDATES_WIDTH: u64 = 64; // Adjust this based on your logic

pub fn decode_011_diff(fact_input_sn_output: &mut Vec<U256>) -> Result<StateDiff, Box<dyn std::error::Error>> {
	let mut offset = 0;
	let num_contract_updates = fact_input_sn_output[offset].low_u64();
	offset += 1;

	let mut nonces: IndexMap<ContractAddress, Nonce> = IndexMap::new();
	let mut deployed_contracts: IndexMap<ContractAddress, ClassHash> = IndexMap::new();
	let mut declared_classes: IndexMap<ClassHash, (CompiledClassHash, ContractClass)> = IndexMap::new();
	let mut replaced_classes: IndexMap<ContractAddress, ClassHash> = IndexMap::new();
	let mut storage_diffs: IndexMap<ContractAddress, IndexMap<StorageKey, StarkFelt>> = IndexMap::new();
	let mut deprecated_declared_classes: IndexMap<ClassHash, DeprecatedContractClass> = IndexMap::new();

	for _ in 0..num_contract_updates {
		// todo: address implement ContractIsDeployed
		let mut sh = [0u8; 32];
		fact_input_sn_output[offset].to_big_endian(&mut sh);
		let address = ContractAddress::try_from(StarkHash::new(sh)?)?;
		offset += 1;

		let summary = fact_input_sn_output[offset];
		offset += 1;

		let num_storage_updates = summary.low_u64();
		// todo 128 whether to change it to a constant
		let class_info_flag = summary.bit(128);
		// Rsh sets z = x >> n and returns z.
		// numStorageUpdatesWidth = 64
		let nonces_value = summary >> 64;
		match Felt252Wrapper::try_from(nonces_value) {
			Ok(contract_address) => {
				nonces.insert(address, Nonce::from(contract_address));
			}
			Err(err) => {
				// handle err
				panic!("Error converting nonces_value: {:?}", err);
			}
		}
		if class_info_flag {
			let class_hash = ClassHash::from(Felt252Wrapper::try_from(fact_input_sn_output[offset]).unwrap());
			offset += 1;
			// todo: address implement ContractIsDeployed
			replaced_classes.insert(address, class_hash);
			// todo: address implement ContractIsDeployed
			// deployed_contracts.insert(address, class_hash);
		}

		if num_storage_updates > 0 {
			let mut diffs = IndexMap::new();
			for _ in 0..num_storage_updates {
				let mut sk = [0u8; 32];
				fact_input_sn_output[offset].to_big_endian(&mut sk);
				let key = StorageKey::try_from(StarkHash::new(sk)?)?;
				// let key = StorageKey::try_from(PatriciaKey::from(fact_input_sn_output[offset]))?;
				offset += 1;
				let mut sf = [0u8; 32];
				fact_input_sn_output[offset].to_big_endian(&mut sf);
				let value = StarkFelt::new(sf)?;
				// let value = StarkFelt::try_from(StarkHash::new(sf).unwrap())?;
				// let value = StarkFelt::from(fact_input_sn_output[offset]);
				offset += 1;
				diffs.insert(key, value);
			}
			storage_diffs.insert(address, diffs);
		}
	}

	let num_declared_classes = fact_input_sn_output[offset].low_u64();
	offset += 1;
	for _ in 0..num_declared_classes {
		let class_hash = ClassHash::from(Felt252Wrapper::try_from(fact_input_sn_output[offset])?);
		offset += 1;
		let compiled_class_hash = CompiledClassHash::from(Felt252Wrapper::try_from(fact_input_sn_output[offset])?);
		offset += 1;
		// todo ContractClass::new() ???
		// declared_classes.insert(class_hash, (compiled_class_hash, ContractClass::new()));
	}

	Ok(StateDiff {
		deployed_contracts,
		storage_diffs,
		declared_classes,
		deprecated_declared_classes,
		nonces,
		replaced_classes,
	})
}

pub fn decode_pre_011_diff(fact_input_sn_output: &mut Vec<U256>, with_constructor_args: bool) -> Result<StateDiff, Box<dyn std::error::Error>> {
	let mut offset = 0;
	let num_deployments_cells = fact_input_sn_output[offset].as_usize();
	offset += 1;

	let mut deployed_contracts: IndexMap<ContractAddress, ClassHash> = IndexMap::new();
	let mut nonces: IndexMap<ContractAddress, Nonce> = IndexMap::new();
	let mut declared_classes: IndexMap<ClassHash, (CompiledClassHash, ContractClass)> = IndexMap::new();
	let mut replaced_classes: IndexMap<ContractAddress, ClassHash> = IndexMap::new();
	let mut storage_diffs: IndexMap<ContractAddress, IndexMap<StorageKey, StarkFelt>> = IndexMap::new();
	let mut deprecated_declared_classes: IndexMap<ClassHash, DeprecatedContractClass> = IndexMap::new();

	// Parse contract deployments
	while offset - 1 < num_deployments_cells {
		let mut sh = [0u8; 32];
		fact_input_sn_output[offset].to_big_endian(&mut sh);
		let address = ContractAddress::try_from(StarkHash::new(sh)?)?;


		offset += 1;
		let class_hash = ClassHash::from(Felt252Wrapper::try_from(fact_input_sn_output[offset]).unwrap());

		offset += 1;
		deployed_contracts.insert(address.clone(), class_hash.clone());

		if with_constructor_args {
			let constructor_args_len = fact_input_sn_output[offset].as_usize();
			offset += 1;
			offset += constructor_args_len ;//as usize;
		}
	}

// Parse contract storage updates
	let updates_len = fact_input_sn_output[offset].as_usize();
	offset += 1;
	for  _i in 0..updates_len {
		let mut sh = [0u8; 32];
		fact_input_sn_output[offset].to_big_endian(&mut sh);
		let address = ContractAddress::try_from(StarkHash::new(sh)?)?;
		offset += 1;
		let num_updates = fact_input_sn_output[offset].low_u64();

		match Felt252Wrapper::try_from(fact_input_sn_output[offset] >> NUM_STORAGE_UPDATES_WIDTH) {
			Ok(contract_address) => {
				nonces.insert(address, Nonce::from(contract_address));
			}
			Err(err) => {
				// handle err
				panic!("Error converting nonces_value: {:?}", err);
			}
		}
		offset += 1;

		let mut diffs = IndexMap::new();
		for _ in 0..num_updates {
			let mut sk = [0u8; 32];
			fact_input_sn_output[offset].to_big_endian(&mut sk);
			let key = StorageKey::try_from(StarkHash::new(sk)?)?;

			offset += 1;
			let mut sf = [0u8; 32];
			fact_input_sn_output[offset].to_big_endian(&mut sf);
			let value = StarkFelt::new(sf)?;
			offset += 1;
			diffs.insert(key.clone(), value.clone());
		}
		storage_diffs.insert(address.clone(), diffs);
	}

	Ok( StateDiff {
		deployed_contracts,
		storage_diffs,
		declared_classes: declared_classes,
		deprecated_declared_classes: deprecated_declared_classes,
		nonces,
		replaced_classes: replaced_classes,
	})
}

// test
#[cfg(test)]
mod tests {
	use super::*;
	use std::convert::TryFrom;
	use mp_felt::Felt252Wrapper;
	use starknet_api::hash::StarkFelt;
	use starknet_api::state::{StateDiff, StorageKey, ContractClass};
	use indexmap::IndexMap;
	use starknet_api::api_core::{
		ClassHash, CompiledClassHash, ContractAddress, EntryPointSelector, GlobalRoot, Nonce,
		PatriciaKey,
	};
	use starknet_api::deprecated_contract_class::ContractClass as DeprecatedContractClass;


	#[tokio::test]
	async fn test_decode_011_diff() {
		let mut fact_input_sn_output: Vec<U256> = vec![
			U256::from(1),
			U256::from(1),
			U256::from(1),
			U256::from(1),
			U256::from(1),
			U256::from(1),
			U256::from(1),
			U256::from(1),
			U256::from(1),
			U256::from(1),
			U256::from(1),
			U256::from(1),
			U256::from(1),
		];
		let state_diff = decode_011_diff(&mut fact_input_sn_output).unwrap();
		println!("{:?}", state_diff);
	}

	#[tokio::test]
	async fn test_decode_pre_011_diff() {
		let mut fact_input_sn_output: Vec<U256> = vec![
			U256::from(1),
			U256::from(2),
			U256::from(3),
			U256::from(1),
			U256::from(5),
			U256::from(6),
			U256::from(7),
			U256::from(8),
			U256::from(9),
			U256::from(10),
			U256::from(11),
			U256::from(12),
			U256::from(13),
			U256::from(14),
			U256::from(15),
			U256::from(16),
			U256::from(17),
		];
		let state_diff = decode_pre_011_diff(&mut fact_input_sn_output, true).unwrap();
		println!("{:?}", state_diff);
	}

    macro_rules! u256_array {
        ($($val:expr),*) => {{
            [$(U256::from_dec_str($val).unwrap()),*]
        }};
    }

    #[test]
    fn test_decode_pre_011_diff_v2(){
        let data = u256_array![
            "2",
            "2472939307328371039455977650994226407024607754063562993856224077254594995194",
            "1336043477925910602175429627555369551262229712266217887481529642650907574765",
            "5",
            "2019172390095051323869047481075102003731246132997057518965927979101413600827",
            "18446744073709551617",
            "5",
            "102",
            "2111158214429736260101797453815341265658516118421387314850625535905115418634",
            "2",
            "619473939880410191267127038055308002651079521370507951329266275707625062498",
            "1471584055184889701471507129567376607666785522455476394130774434754411633091",
            "619473939880410191267127038055308002651079521370507951329266275707625062499",
            "541081937647750334353499719661793404023294520617957763260656728924567461866",
            "2472939307328371039455977650994226407024607754063562993856224077254594995194",
            "1",
            "955723665991825982403667749532843665052270105995360175183368988948217233556",
            "2439272289032330041885427773916021390926903450917097317807468082958581062272",
            "3429319713503054399243751728532349500489096444181867640228809233993992987070",
            "1",
            "5",
            "1110",
            "3476138891838001128614704553731964710634238587541803499001822322602421164873",
            "6",
            "59664015286291125586727181187045849528930298741728639958614076589374875456",
            "600",
            "221246409693049874911156614478125967098431447433028390043893900771521609973",
            "400",
            "558404273560404778508455254030458021013656352466216690688595011803280448030",
            "100",
            "558404273560404778508455254030458021013656352466216690688595011803280448031",
            "200",
            "558404273560404778508455254030458021013656352466216690688595011803280448032",
            "300",
            "1351148242645005540004162531550805076995747746087542030095186557536641755046",
            "500"
          ];

        let state_diff = decode_pre_011_diff(&mut data.to_vec(), false).unwrap();
        
        println!("{:#?}", state_diff);
    }
}
