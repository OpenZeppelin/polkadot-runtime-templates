use std::path::PathBuf;

use sc_cli::{ChainSpec, Result};
use sc_network::config::NetworkConfiguration;

use crate::{contracts::ContractsPath, eth::EthConfiguration};

/// Sub-commands supported by the collator.
#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Build a chain specification.
    BuildSpec(ExtendedBuildSpecCmd),

    /// Validate blocks.
    CheckBlock(sc_cli::CheckBlockCmd),

    /// Export blocks.
    ExportBlocks(sc_cli::ExportBlocksCmd),

    /// Export the state of a given block into a chain spec.
    ExportState(sc_cli::ExportStateCmd),

    /// Import blocks.
    ImportBlocks(sc_cli::ImportBlocksCmd),

    /// Revert the chain to a previous state.
    Revert(sc_cli::RevertCmd),

    /// Remove the whole chain.
    PurgeChain(cumulus_client_cli::PurgeChainCmd),

    /// Export the genesis head data of the parachain.
    ///
    /// Head data is the encoded block header.
    #[command(alias = "export-genesis-state")]
    ExportGenesisHead(cumulus_client_cli::ExportGenesisHeadCommand),

    /// Export the genesis wasm of the parachain.
    ExportGenesisWasm(cumulus_client_cli::ExportGenesisWasmCommand),

    /// Sub-commands concerned with benchmarking.
    /// The pallet benchmarking moved to the `pallet` sub-command.
    #[command(subcommand)]
    Benchmark(frame_benchmarking_cli::BenchmarkCmd),

    /// Try-runtime has migrated to a standalone
    /// [CLI](<https://github.com/paritytech/try-runtime-cli>). The subcommand exists as a stub and
    /// deprecation notice. It will be removed entirely some time after January
    /// 2024.
    TryRuntime,
}

impl Subcommand {
    pub fn contract_directory(&self) -> ContractsPath {
        match self {
            Self::BuildSpec(cmd) =>
                match (cmd.no_predeployed_contracts, cmd.predeployed_contracts.clone()) {
                    (true, _) => ContractsPath::None,
                    (false, None) => ContractsPath::Default,
                    (false, Some(path)) => ContractsPath::Some(path),
                },
            _ => ContractsPath::None,
        }
    }
}

#[derive(Debug, Clone, clap::Parser)]
pub struct ExtendedBuildSpecCmd {
    #[clap(flatten)]
    pub cmd: sc_cli::BuildSpecCmd,

    /// Path to a directory that contains precompiled contracts.
    /// A directory should contain file `contracts.json`.
    /// It should contain a JSON array of objects with "address" and "filename" files in it. See the example in `evm-template/contracts/contracts.json`
    /// If you don't specify any contract, only Entrypoint contract will be deployed at the address `0x81ead4918134AE386dbd04346216E20AB8F822C4`
    #[arg(long, default_value = None)]
    pub predeployed_contracts: Option<String>,

    /// Set this to true to if you do not want any contracts to be deployed
    #[arg(long, default_value = "false")]
    pub no_predeployed_contracts: bool,
}

impl ExtendedBuildSpecCmd {
    /// Run the build-spec command
    pub fn run(
        &self,
        spec: Box<dyn ChainSpec>,
        network_config: NetworkConfiguration,
    ) -> Result<()> {
        self.cmd.run(spec, network_config)
    }
}

const AFTER_HELP_EXAMPLE: &str = color_print::cstr!(
    r#"<bold><underline>Examples:</></>
   <bold>parachain-template-node build-spec --disable-default-bootnode > plain-parachain-chainspec.json</>
           Export a chainspec for a local testnet in json format.
   <bold>parachain-template-node --chain plain-parachain-chainspec.json --tmp -- --chain paseo-local</>
           Launch a full node with chain specification loaded from plain-parachain-chainspec.json.
   <bold>parachain-template-node</>
           Launch a full node with default parachain <italic>local-testnet</> and relay chain <italic>paseo-local</>.
   <bold>parachain-template-node --collator</>
           Launch a collator with default parachain <italic>local-testnet</> and relay chain <italic>paseo-local</>.
 "#
);
#[derive(Debug, clap::Parser)]
#[command(
    propagate_version = true,
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
#[clap(after_help = AFTER_HELP_EXAMPLE)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[command(flatten)]
    pub run: cumulus_client_cli::RunCmd,

    /// Disable automatic hardware benchmarks.
    ///
    /// By default these benchmarks are automatically ran at startup and measure
    /// the CPU speed, the memory bandwidth and the disk speed.
    ///
    /// The results are then printed out in the logs, and also sent as part of
    /// telemetry, if telemetry is enabled.
    #[arg(long)]
    pub no_hardware_benchmarks: bool,

    /// Relay chain arguments
    #[arg(raw = true)]
    pub relay_chain_args: Vec<String>,

    #[command(flatten)]
    pub eth: EthConfiguration,
}

#[derive(Debug)]
pub struct RelayChainCli {
    /// The actual relay chain cli object.
    pub base: polkadot_cli::RunCmd,

    /// Optional chain id that should be passed to the relay chain.
    pub chain_id: Option<String>,

    /// The base path that should be used by the relay chain.
    pub base_path: Option<PathBuf>,
}

impl RelayChainCli {
    /// Parse the relay chain CLI parameters using the para chain
    /// `Configuration`.
    pub fn new<'a>(
        para_config: &sc_service::Configuration,
        relay_chain_args: impl Iterator<Item = &'a String>,
    ) -> Self {
        let extension = crate::chain_spec::Extensions::try_get(&*para_config.chain_spec);
        let chain_id = extension.map(|e| e.relay_chain.clone());
        let base_path = para_config.base_path.path().join("polkadot");
        Self {
            base_path: Some(base_path),
            chain_id,
            base: clap::Parser::parse_from(relay_chain_args),
        }
    }
}
