use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

// Local
use evm_runtime_template::opaque::Block;
use fc_rpc::EthTask;
pub use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
pub use fc_storage::{StorageOverride, StorageOverrideHandler};
use futures::{future, prelude::*};
// Substrate
use sc_client_api::BlockchainEvents;
use sc_executor::{HostFunctions, WasmExecutor};
use sc_network_sync::SyncingService;
use sc_service::{error::Error as ServiceError, Configuration, TaskManager};
use sp_api::ConstructRuntimeApi;

/// Full backend.
pub type FullBackend = sc_service::TFullBackend<Block>;
/// Full client.
pub type FullClient<RuntimeApi, Executor> =
    sc_service::TFullClient<Block, RuntimeApi, WasmExecutor<Executor>>;

/// Frontier DB backend type.
pub type FrontierBackend<C> = fc_db::Backend<Block, C>;

pub fn db_config_dir(config: &Configuration) -> PathBuf {
    config.base_path.config_dir(config.chain_spec.id())
}

/// Avalailable frontier backend types.
#[derive(Debug, Copy, Clone, Default, clap::ValueEnum)]
pub enum BackendType {
    /// Either RocksDb or ParityDb as per inherited from the global backend settings.
    #[default]
    KeyValue,
    /// Sql database with custom log indexing.
    Sql,
}

/// The ethereum-compatibility configuration used to run a node.
#[derive(Clone, Debug, clap::Parser)]
pub struct EthConfiguration {
    /// Maximum number of logs in a query.
    #[arg(long, default_value = "10000")]
    pub max_past_logs: u32,

    /// Maximum fee history cache size.
    #[arg(long, default_value = "2048")]
    pub fee_history_limit: u64,

    #[arg(long)]
    pub enable_dev_signer: bool,

    /// The dynamic-fee pallet target gas price set by block author
    #[arg(long, default_value = "1")]
    pub target_gas_price: u64,

    /// Maximum allowed gas limit will be `block.gas_limit * execute_gas_limit_multiplier`
    /// when using eth_call/eth_estimateGas.
    #[arg(long, default_value = "10")]
    pub execute_gas_limit_multiplier: u64,

    /// Size in bytes of the LRU cache for block data.
    #[arg(long, default_value = "50")]
    pub eth_log_block_cache: usize,

    /// Size in bytes of the LRU cache for transactions statuses data.
    #[arg(long, default_value = "50")]
    pub eth_statuses_cache: usize,

    /// Sets the frontier backend type (KeyValue or Sql)
    #[arg(long, value_enum, ignore_case = true, default_value_t = BackendType::default())]
    pub frontier_backend_type: BackendType,

    // Sets the SQL backend's pool size.
    #[arg(long, default_value = "100")]
    pub frontier_sql_backend_pool_size: u32,

    /// Sets the SQL backend's query timeout in number of VM ops.
    #[arg(long, default_value = "10000000")]
    pub frontier_sql_backend_num_ops_timeout: u32,

    /// Sets the SQL backend's auxiliary thread limit.
    #[arg(long, default_value = "4")]
    pub frontier_sql_backend_thread_count: u32,

    /// Sets the SQL backend's query timeout in number of VM ops.
    /// Default value is 200MB.
    #[arg(long, default_value = "209715200")]
    pub frontier_sql_backend_cache_size: u64,
}

pub struct FrontierPartialComponents {
    pub filter_pool: Option<FilterPool>,
    pub fee_history_cache: FeeHistoryCache,
    pub fee_history_cache_limit: FeeHistoryCacheLimit,
}

pub fn new_frontier_partial(
    config: &EthConfiguration,
) -> Result<FrontierPartialComponents, ServiceError> {
    Ok(FrontierPartialComponents {
        filter_pool: Some(Arc::new(Mutex::new(BTreeMap::new()))),
        fee_history_cache: Arc::new(Mutex::new(BTreeMap::new())),
        fee_history_cache_limit: config.fee_history_limit,
    })
}

/// A set of APIs that ethereum-compatible runtimes must implement.
pub trait EthCompatRuntimeApiCollection:
	sp_api::ApiExt<Block>
	+ fp_rpc::ConvertTransactionRuntimeApi<Block>
	+ fp_rpc::EthereumRuntimeRPCApi<Block>
{
}

impl<Api> EthCompatRuntimeApiCollection for Api where
	Api: sp_api::ApiExt<Block>
		+ fp_rpc::ConvertTransactionRuntimeApi<Block>
		+ fp_rpc::EthereumRuntimeRPCApi<Block>
{
}

pub async fn spawn_frontier_tasks<RuntimeApi, Executor>(
    task_manager: &TaskManager,
    client: Arc<FullClient<RuntimeApi, Executor>>,
    backend: Arc<FullBackend>,
    frontier_backend: Arc<FrontierBackend<FullClient<RuntimeApi, Executor>>>,
    filter_pool: Option<FilterPool>,
    storage_override: Arc<dyn StorageOverride<Block>>,
    fee_history_cache: FeeHistoryCache,
    fee_history_cache_limit: FeeHistoryCacheLimit,
    sync: Arc<SyncingService<Block>>,
    pubsub_notification_sinks: Arc<
        fc_mapping_sync::EthereumBlockNotificationSinks<
            fc_mapping_sync::EthereumBlockNotification<Block>,
        >,
    >,
) where
    RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>>,
    RuntimeApi: Send + Sync + 'static,
    RuntimeApi::RuntimeApi: EthCompatRuntimeApiCollection,
    Executor: HostFunctions + 'static,
{
    // Spawn main mapping sync worker background task.
    match &*frontier_backend {
		fc_db::Backend::KeyValue(b) => {
			task_manager.spawn_essential_handle().spawn(
				"frontier-mapping-sync-worker",
				Some("frontier"),
				fc_mapping_sync::kv::MappingSyncWorker::new(
					client.import_notification_stream(),
					Duration::new(6, 0),
					client.clone(),
					backend,
					storage_override.clone(),
					b.clone(),
					3,
					0u32.into(),
					fc_mapping_sync::SyncStrategy::Normal,
					sync,
					pubsub_notification_sinks,
				)
				.for_each(|()| future::ready(())),
			);
		}
		fc_db::Backend::Sql(b) => {
			task_manager.spawn_essential_handle().spawn_blocking(
				"frontier-mapping-sync-worker",
				Some("frontier"),
				fc_mapping_sync::sql::SyncWorker::run(
					client.clone(),
					backend,
					b.clone(),
					client.import_notification_stream(),
					fc_mapping_sync::sql::SyncWorkerConfig {
						read_notification_timeout: Duration::from_secs(30),
						check_indexed_blocks_interval: Duration::from_secs(60),
					},
					fc_mapping_sync::SyncStrategy::Parachain,
					sync,
					pubsub_notification_sinks,
				),
			);
		}
	}

    // Spawn Frontier EthFilterApi maintenance task.
    if let Some(filter_pool) = filter_pool {
        // Each filter is allowed to stay in the pool for 100 blocks.
        const FILTER_RETAIN_THRESHOLD: u64 = 100;
        task_manager.spawn_essential_handle().spawn(
            "frontier-filter-pool",
            Some("frontier"),
            EthTask::filter_pool_task(client.clone(), filter_pool, FILTER_RETAIN_THRESHOLD),
        );
    }

    // Spawn Frontier FeeHistory cache maintenance task.
    task_manager.spawn_essential_handle().spawn(
        "frontier-fee-history",
        Some("frontier"),
        EthTask::fee_history_task(
            client,
            storage_override,
            fee_history_cache,
            fee_history_cache_limit,
        ),
    );
}
