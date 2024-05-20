//! Service and ServiceFactory implementation. Specialized wrapper over
//! substrate service.

// std
use std::{path::Path, sync::Arc, time::Duration};

use cumulus_client_cli::CollatorOptions;
// Cumulus Imports
use cumulus_client_collator::service::CollatorService;
use cumulus_client_consensus_common::ParachainBlockImport as TParachainBlockImport;
use cumulus_client_consensus_proposer::Proposer;
use cumulus_client_service::{
    build_network, build_relay_chain_interface, prepare_node_config, start_relay_chain_tasks,
    BuildNetworkParams, CollatorSybilResistance, DARecoveryProfile, StartRelayChainTasksParams,
};
#[cfg(feature = "async-backing")]
use cumulus_primitives_core::relay_chain::ValidationCode;
use cumulus_primitives_core::{relay_chain::CollatorPair, ParaId};
use cumulus_relay_chain_interface::{OverseerHandle, RelayChainInterface};
// Substrate Imports
use frame_benchmarking_cli::SUBSTRATE_REFERENCE_HARDWARE;
// Local Runtime Types
use parachain_template_runtime::{
    opaque::{Block, Hash},
    RuntimeApi, TransactionConverter,
};
use sc_client_api::Backend;
use sc_consensus::ImportQueue;
use sc_executor::{
    HeapAllocStrategy, NativeElseWasmExecutor, WasmExecutor, DEFAULT_HEAP_ALLOC_STRATEGY,
};
use sc_network::NetworkBlock;
use sc_network_sync::SyncingService;
use sc_service::{Configuration, PartialComponents, TFullBackend, TFullClient, TaskManager};
use sc_telemetry::{Telemetry, TelemetryHandle, TelemetryWorker, TelemetryWorkerHandle};
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sp_core::U256;
use sp_keystore::KeystorePtr;
use substrate_prometheus_endpoint::Registry;

use crate::eth::{db_config_dir, new_frontier_partial, spawn_frontier_tasks, BackendType, EthConfiguration, FrontierBackend, FrontierPartialComponents};

/// Native executor type.
pub struct ParachainNativeExecutor;

impl sc_executor::NativeExecutionDispatch for ParachainNativeExecutor {
    type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

    fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
        parachain_template_runtime::api::dispatch(method, data)
    }

    fn native_version() -> sc_executor::NativeVersion {
        parachain_template_runtime::native_version()
    }
}

type ParachainExecutor = NativeElseWasmExecutor<ParachainNativeExecutor>;

type ParachainClient = TFullClient<Block, RuntimeApi, ParachainExecutor>;

type ParachainBackend = TFullBackend<Block>;

type ParachainBlockImport = TParachainBlockImport<Block, Arc<ParachainClient>, ParachainBackend>;

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the
/// builder in order to be able to perform chain operations.
pub fn new_partial(
    config: &Configuration,
	eth_config: &EthConfiguration,
) -> Result<
    PartialComponents<
        ParachainClient,
        ParachainBackend,
        (),
        sc_consensus::DefaultImportQueue<Block>,
        sc_transaction_pool::FullPool<Block, ParachainClient>,
        (ParachainBlockImport, Option<Telemetry>, Option<TelemetryWorkerHandle>, FrontierBackend, Arc<fc_rpc::OverrideHandle<Block>>),
    >,
    sc_service::Error,
> {
    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let heap_pages = config
        .default_heap_pages
        .map_or(DEFAULT_HEAP_ALLOC_STRATEGY, |h| HeapAllocStrategy::Static { extra_pages: h as _ });

    let wasm = WasmExecutor::builder()
        .with_execution_method(config.wasm_method)
        .with_onchain_heap_alloc_strategy(heap_pages)
        .with_offchain_heap_alloc_strategy(heap_pages)
        .with_max_runtime_instances(config.max_runtime_instances)
        .with_runtime_cache_size(config.runtime_cache_size)
        .build();

    let executor = ParachainExecutor::new_with_wasm_executor(wasm);

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )?;
    let client = Arc::new(client);

    let telemetry_worker_handle = telemetry.as_ref().map(|(worker, _)| worker.handle());

    let telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager.spawn_handle().spawn("telemetry", None, worker.run());
        telemetry
    });

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_essential_handle(),
        client.clone(),
    );

    let block_import = ParachainBlockImport::new(client.clone(), backend.clone());

    let import_queue = build_import_queue(
        client.clone(),
        block_import.clone(),
        config,
        telemetry.as_ref().map(|telemetry| telemetry.handle()),
        &task_manager,
    )?;

	let overrides = crate::rpc::overrides_handle(client.clone());
    let dbcf = &config.database;
    let frontier_backend = match eth_config.frontier_backend_type {
		BackendType::KeyValue => FrontierBackend::KeyValue(fc_db::kv::Backend::open(
			Arc::clone(&client),
			&config.database,
			&db_config_dir(config),
		)?),
		BackendType::Sql => {
			let db_path = db_config_dir(config).join("sql");
			std::fs::create_dir_all(&db_path).expect("failed creating sql db directory");
			let backend = futures::executor::block_on(fc_db::sql::Backend::new(
				fc_db::sql::BackendConfig::Sqlite(fc_db::sql::SqliteBackendConfig {
					path: Path::new("sqlite:///")
						.join(db_path)
						.join("frontier.db3")
						.to_str()
						.unwrap(),
					create_if_missing: true,
					thread_count: eth_config.frontier_sql_backend_thread_count,
					cache_size: eth_config.frontier_sql_backend_cache_size,
				}),
				eth_config.frontier_sql_backend_pool_size,
				std::num::NonZeroU32::new(eth_config.frontier_sql_backend_num_ops_timeout),
				overrides.clone(),
			))
			.unwrap_or_else(|err| panic!("failed creating sql backend: {:?}", err));
			FrontierBackend::Sql(backend)
		}
	};

    Ok(PartialComponents {
        backend,
        client,
        import_queue,
        keystore_container,
        task_manager,
        transaction_pool,
        select_chain: (),
        other: (block_import, telemetry, telemetry_worker_handle, frontier_backend, overrides),
    })
}

/// Start a node with the given parachain `Configuration` and relay chain
/// `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the
/// runtime api.
#[sc_tracing::logging::prefix_logs_with("Parachain")]
async fn start_node_impl(
    parachain_config: Configuration,
    polkadot_config: Configuration,
    collator_options: CollatorOptions,
	eth_config: &EthConfiguration,
    para_id: ParaId,
    hwbench: Option<sc_sysinfo::HwBench>,
) -> sc_service::error::Result<(TaskManager, Arc<ParachainClient>)> {
    let parachain_config = prepare_node_config(parachain_config);

    let params = new_partial(&parachain_config, eth_config)?;

    let FrontierPartialComponents {
		filter_pool,
		fee_history_cache,
		fee_history_cache_limit,
	} = new_frontier_partial(&eth_config)?;

    let (block_import, mut telemetry, telemetry_worker_handle, frontier_backend, overrides) = params.other;
    let net_config = sc_network::config::FullNetworkConfiguration::new(&parachain_config.network);

    let client = params.client.clone();
    let backend = params.backend.clone();
    let mut task_manager = params.task_manager;

    let (relay_chain_interface, collator_key) = build_relay_chain_interface(
        polkadot_config,
        &parachain_config,
        telemetry_worker_handle,
        &mut task_manager,
        collator_options.clone(),
        hwbench.clone(),
    )
    .await
    .map_err(|e| sc_service::Error::Application(Box::new(e) as Box<_>))?;

    let validator = parachain_config.role.is_authority();
    let prometheus_registry = parachain_config.prometheus_registry().cloned();
    let transaction_pool = params.transaction_pool.clone();
    let import_queue_service = params.import_queue.service();
    let slot_duration = sc_consensus_aura::slot_duration(&*client)?;

    let (network, system_rpc_tx, tx_handler_controller, start_network, sync_service) =
        build_network(BuildNetworkParams {
            parachain_config: &parachain_config,
            net_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            para_id,
            spawn_handle: task_manager.spawn_handle(),
            relay_chain_interface: relay_chain_interface.clone(),
            import_queue: params.import_queue,
            sybil_resistance_level: CollatorSybilResistance::Resistant, // because of Aura
        })
        .await?;

    if parachain_config.offchain_worker.enabled {
        use futures::FutureExt;

        task_manager.spawn_handle().spawn(
            "offchain-workers-runner",
            "offchain-work",
            sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
                runtime_api_provider: client.clone(),
                keystore: Some(params.keystore_container.keystore()),
                offchain_db: backend.offchain_storage(),
                transaction_pool: Some(OffchainTransactionPoolFactory::new(
                    transaction_pool.clone(),
                )),
                network_provider: network.clone(),
                is_validator: parachain_config.role.is_authority(),
                enable_http_requests: false,
                custom_extensions: move |_| vec![],
            })
            .run(client.clone(), task_manager.spawn_handle())
            .boxed(),
        );
    }

    let pubsub_notification_sinks: fc_mapping_sync::EthereumBlockNotificationSinks<
        fc_mapping_sync::EthereumBlockNotification<Block>,
    > = Default::default();
    let pubsub_notification_sinks = Arc::new(pubsub_notification_sinks);

    let rpc_builder = {
        let client = client.clone();
        let transaction_pool = transaction_pool.clone();
        let target_gas_price = eth_config.target_gas_price;
		let enable_dev_signer = eth_config.enable_dev_signer;
        let pending_create_inherent_data_providers = move |_, ()| async move {
			let current = sp_timestamp::InherentDataProvider::from_system_time();
			let next_slot = current.timestamp().as_millis() + slot_duration.as_millis();
			let timestamp = sp_timestamp::InherentDataProvider::new(next_slot.into());
			let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
				*timestamp,
				slot_duration, 
			);
			let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));
			Ok((slot, timestamp, dynamic_fee))
		};
        let block_data_cache = Arc::new(fc_rpc::EthBlockDataCacheTask::new(
			task_manager.spawn_handle(),
			overrides.clone(),
			eth_config.eth_log_block_cache,
			eth_config.eth_statuses_cache,
			prometheus_registry.clone(),
		));
		let execute_gas_limit_multiplier = eth_config.execute_gas_limit_multiplier;
		let max_past_logs = eth_config.max_past_logs;
        let network = network.clone();
        let sync_service = sync_service.clone();
        let frontier_backend = frontier_backend.clone();
        let filter_pool = filter_pool.clone();
        let overrides = overrides.clone();
        let fee_history_cache = fee_history_cache.clone();
        let pubsub_notification_sinks = pubsub_notification_sinks.clone();

        Box::new(move |deny_unsafe, subscription_task_executor| {
            let eth = crate::rpc::EthDeps {
                client: client.clone(),
                pool: transaction_pool.clone(),
                graph: transaction_pool.pool().clone(),
                converter: Some(TransactionConverter),
                is_authority: validator,
                enable_dev_signer,
                network: network.clone(),
                sync: sync_service.clone(),
                frontier_backend: match frontier_backend.clone() {
					fc_db::Backend::KeyValue(b) => Arc::new(b),
					fc_db::Backend::Sql(b) => Arc::new(b),
				},
                overrides: overrides.clone(),
                block_data_cache: block_data_cache.clone(),
                filter_pool: filter_pool.clone(),
                max_past_logs,
                fee_history_cache: fee_history_cache.clone(),
                fee_history_cache_limit,
                execute_gas_limit_multiplier,
                forced_parent_hashes: None,
                pending_create_inherent_data_providers
            };
            let deps = crate::rpc::FullDeps {
                client: client.clone(),
                pool: transaction_pool.clone(),
                deny_unsafe,
                eth
            };

            crate::rpc::create_full(deps, subscription_task_executor, pubsub_notification_sinks.clone()).map_err(Into::into)
        })
    };

    sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        rpc_builder,
        client: client.clone(),
        transaction_pool: transaction_pool.clone(),
        task_manager: &mut task_manager,
        config: parachain_config,
        keystore: params.keystore_container.keystore(),
        backend: backend.clone(),
        network: network.clone(),
        sync_service: sync_service.clone(),
        system_rpc_tx,
        tx_handler_controller,
        telemetry: telemetry.as_mut(),
    })?;

    if let Some(hwbench) = hwbench {
        sc_sysinfo::print_hwbench(&hwbench);
        // Here you can check whether the hardware meets your chains' requirements.
        // Putting a link in there and swapping out the requirements for your
        // own are probably a good idea. The requirements for a para-chain are
        // dictated by its relay-chain.
        match SUBSTRATE_REFERENCE_HARDWARE.check_hardware(&hwbench) {
            Err(err) if validator => {
                log::warn!(
                    "⚠️  The hardware does not meet the minimal requirements {} for role \
                     'Authority'.",
                    err
                );
            }
            _ => {}
        }

        if let Some(ref mut telemetry) = telemetry {
            let telemetry_handle = telemetry.handle();
            task_manager.spawn_handle().spawn(
                "telemetry_hwbench",
                None,
                sc_sysinfo::initialize_hwbench_telemetry(telemetry_handle, hwbench),
            );
        }
    }

    let announce_block = {
        let sync_service = sync_service.clone();
        Arc::new(move |hash, data| sync_service.announce_block(hash, data))
    };

    let relay_chain_slot_duration = Duration::from_secs(6);

    let overseer_handle = relay_chain_interface
        .overseer_handle()
        .map_err(|e| sc_service::Error::Application(Box::new(e)))?;

    start_relay_chain_tasks(StartRelayChainTasksParams {
        client: client.clone(),
        announce_block: announce_block.clone(),
        para_id,
        relay_chain_interface: relay_chain_interface.clone(),
        task_manager: &mut task_manager,
        da_recovery_profile: if validator {
            DARecoveryProfile::Collator
        } else {
            DARecoveryProfile::FullNode
        },
        import_queue: import_queue_service,
        relay_chain_slot_duration,
        recovery_handle: Box::new(overseer_handle.clone()),
        sync_service: sync_service.clone(),
    })?;

    spawn_frontier_tasks(
		&task_manager,
		client.clone(),
		backend.clone(),
		frontier_backend,
		filter_pool,
		overrides,
		fee_history_cache,
		fee_history_cache_limit,
		sync_service.clone(),
		pubsub_notification_sinks,
	)
	.await;

    if validator {
        start_consensus(
            client.clone(),
            #[cfg(feature = "async-backing")]
            backend.clone(),
            block_import,
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|t| t.handle()),
            &task_manager,
            relay_chain_interface.clone(),
            transaction_pool,
            sync_service.clone(),
            params.keystore_container.keystore(),
            relay_chain_slot_duration,
            para_id,
            collator_key.expect("Command line arguments do not allow this. qed"),
            overseer_handle,
            announce_block,
        )?;
    }

    start_network.start_network();

    Ok((task_manager, client))
}

/// Build the import queue for the parachain runtime.
fn build_import_queue(
    client: Arc<ParachainClient>,
    block_import: ParachainBlockImport,
    config: &Configuration,
    telemetry: Option<TelemetryHandle>,
    task_manager: &TaskManager,
) -> Result<sc_consensus::DefaultImportQueue<Block>, sc_service::Error> {
    let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

    Ok(cumulus_client_consensus_aura::equivocation_import_queue::fully_verifying_import_queue::<
        sp_consensus_aura::sr25519::AuthorityPair,
        _,
        _,
        _,
        _,
    >(
        client,
        block_import,
        move |_, _| async move {
            let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
            Ok(timestamp)
        },
        slot_duration,
        &task_manager.spawn_essential_handle(),
        config.prometheus_registry(),
        telemetry,
    ))
}

fn start_consensus(
    client: Arc<ParachainClient>,
    #[cfg(feature = "async-backing")] backend: Arc<ParachainBackend>,
    block_import: ParachainBlockImport,
    prometheus_registry: Option<&Registry>,
    telemetry: Option<TelemetryHandle>,
    task_manager: &TaskManager,
    relay_chain_interface: Arc<dyn RelayChainInterface>,
    transaction_pool: Arc<sc_transaction_pool::FullPool<Block, ParachainClient>>,
    sync_oracle: Arc<SyncingService<Block>>,
    keystore: KeystorePtr,
    relay_chain_slot_duration: Duration,
    para_id: ParaId,
    collator_key: CollatorPair,
    overseer_handle: OverseerHandle,
    announce_block: Arc<dyn Fn(Hash, Option<Vec<u8>>) + Send + Sync>,
) -> Result<(), sc_service::Error> {
    #[cfg(not(feature = "async-backing"))]
    use cumulus_client_consensus_aura::collators::basic::{self as basic_aura, Params};
    #[cfg(feature = "async-backing")]
    use cumulus_client_consensus_aura::collators::lookahead::{self as aura, Params};

    // NOTE: because we use Aura here explicitly, we can use
    // `CollatorSybilResistance::Resistant` when starting the network.
    let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

    let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
        task_manager.spawn_handle(),
        client.clone(),
        transaction_pool,
        prometheus_registry,
        telemetry.clone(),
    );

    let proposer = Proposer::new(proposer_factory);

    let collator_service = CollatorService::new(
        client.clone(),
        Arc::new(task_manager.spawn_handle()),
        announce_block,
        client.clone(),
    );

    let params = Params {
        create_inherent_data_providers: move |_, ()| async move { Ok(()) },
        block_import,
        #[cfg(not(feature = "async-backing"))]
        para_client: client,
        #[cfg(feature = "async-backing")]
        para_client: client.clone(),
        #[cfg(feature = "async-backing")]
        para_backend: backend,
        relay_client: relay_chain_interface,
        #[cfg(feature = "async-backing")]
        code_hash_provider: move |block_hash| {
            client.code_at(block_hash).ok().map(ValidationCode).map(|c| c.hash())
        },
        sync_oracle,
        keystore,
        collator_key,
        para_id,
        overseer_handle,
        slot_duration,
        relay_chain_slot_duration,
        proposer,
        collator_service,
        // Very limited proposal time.
        #[cfg(not(feature = "async-backing"))]
        authoring_duration: Duration::from_millis(500),
        #[cfg(feature = "async-backing")]
        authoring_duration: Duration::from_millis(1500),
        #[cfg(not(feature = "async-backing"))]
        collation_request_receiver: None,
        #[cfg(feature = "async-backing")]
        reinitialize: false,
    };

    #[cfg(not(feature = "async-backing"))]
    let fut =
        basic_aura::run::<Block, sp_consensus_aura::sr25519::AuthorityPair, _, _, _, _, _, _, _>(
            params,
        );
    #[cfg(feature = "async-backing")]
    let fut =
        aura::run::<Block, sp_consensus_aura::sr25519::AuthorityPair, _, _, _, _, _, _, _, _, _>(
            params,
        );
    task_manager.spawn_essential_handle().spawn("aura", None, fut);

    Ok(())
}

/// Start a parachain node.
pub async fn start_parachain_node(
    parachain_config: Configuration,
    polkadot_config: Configuration,
    collator_options: CollatorOptions,
	eth_config: &EthConfiguration,
    para_id: ParaId,
    hwbench: Option<sc_sysinfo::HwBench>,
) -> sc_service::error::Result<(TaskManager, Arc<ParachainClient>)> {
    start_node_impl(parachain_config, polkadot_config, collator_options, eth_config, para_id, hwbench).await
}
