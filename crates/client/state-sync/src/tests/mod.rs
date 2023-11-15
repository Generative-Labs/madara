//! Madara client testing utilities.

pub use substrate_test_client::*;
pub use madara_runtime::{WASM_BINARY, GenesisConfig, BuildStorage, SystemConfig};
pub use madara_runtime as runtime;
use pallet_starknet::genesis_loader::{GenesisData, GenesisLoader};

pub type Backend = sc_client_db::Backend<runtime::Block>;

pub struct MadaraExecutorDispatch;
impl sc_executor::NativeExecutionDispatch for MadaraExecutorDispatch {
    /// Only enable the benchmarking host functions when we actually want to benchmark.
    #[cfg(feature = "runtime-benchmarks")]
    type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
    /// Otherwise we only use the default Substrate host functions.
    #[cfg(not(feature = "runtime-benchmarks"))]
    type ExtendHostFunctions = ();

    fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
        madara_runtime::api::dispatch(method, data)
    }

    fn native_version() -> sc_executor::NativeVersion {
        madara_runtime::native_version()
    }
}

pub type ExecutorDispatch = sc_executor::NativeElseWasmExecutor<MadaraExecutorDispatch>;

/// Test client type.
pub type Client = client::Client<
	Backend,
	client::LocalCallExecutor<runtime::Block, Backend, ExecutorDispatch>,
	runtime::Block,
	runtime::RuntimeApi,
>;

#[derive(Default)]
pub struct GenesisParameters;

impl substrate_test_client::GenesisInit for GenesisParameters {
	fn genesis_storage(&self) -> Storage {
        let genesis_data: GenesisData = serde_json::from_str(std::include_str!("./genesis.json")).unwrap();
        let genesis_loader = GenesisLoader::new(project_root::get_project_root().unwrap(), genesis_data);
        
        let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string()).unwrap();
        GenesisConfig {
            system: SystemConfig {
                code: wasm_binary.to_vec(),
            },
            aura: Default::default(),
            grandpa: Default::default(),
            starknet: genesis_loader.into(),
        }.build_storage().unwrap()
	}
}

pub type TestClientBuilder<E, B> = substrate_test_client::TestClientBuilder<
	runtime::Block,
	E,
	B,
	GenesisParameters,
>;

pub trait TestClientBuilderExt: Sized {
	/// Create test client builder.
	fn new() -> Self;

	/// Build the test client.
	fn build(self) -> Client;
}

impl TestClientBuilderExt
	for substrate_test_client::TestClientBuilder<
		runtime::Block,
		client::LocalCallExecutor<runtime::Block, Backend, ExecutorDispatch>,
		Backend,
		GenesisParameters,
	>
{
	fn new() -> Self {
		Self::default()
	}

	fn build(self) -> Client {
		self.build_with_native_executor(None).0
	}
}

#[test]
fn test_basic_state_diff(){
	let mut _client = TestClientBuilder::new().build();
	// 1. make block (transfer, deploy)
	// 2. client new block builder
	// 3. block builder build block, get {block, state changes, applied state root}
	// 4. get state diff from state changes
	// 4. apply state diff to client. 
	// 5. check starknet contract state by runtime api
}