#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate anyhow;

use lib::{
    run_requirements_proceeding, run_telemetry_watcher, Candidate, Network, NodeName,
    RequirementsConfig, RequirementsProceedingConfig, Result, StoreBehavior,
    TelemetryWatcherConfig,
};
use std::fs::read_to_string;
use tokio::time::{self, Duration};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RootConfig {
    db_uri: String,
    db_name: String,
    telemetry_watcher: ConfigWrapper<TelemetryTrackerConfig>,
    candidate_verifier: ConfigWrapper<Vec<CandidateVerifierNetworkConfig>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigWrapper<T> {
    pub enabled: bool,
    pub config: Option<T>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TelemetryTrackerConfig {
    telemetry_host: String,
    store_behavior: StoreBehavior,
    networks: Vec<Network>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CandidateVerifierNetworkConfig {
    network: Network,
    rpc_hostname: String,
    candidate_file: String,
    requirements_config: RequirementsConfig<u128>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RawCandidate {
    name: NodeName,
    stash: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_module("candidate_verifier", log::LevelFilter::Debug)
        .filter_module("lib", log::LevelFilter::Debug)
        .init();

    let root_config: RootConfig = serde_yaml::from_str(&read_to_string("config/service.yml")?)?;

    // Process telemetry tracker configuration.
    let tracker = root_config.telemetry_watcher;
    if tracker.enabled {
        let tracker_config = tracker.config.ok_or(anyhow!(
            "No configuration is provided for (enabled) telemetry tracker"
        ))?;

        for network in tracker_config.networks {
            info!(
                "Starting telemetry watcher for {} network",
                network.as_ref()
            );
            let specialized = TelemetryWatcherConfig {
                db_uri: root_config.db_uri.clone(),
                db_name: root_config.db_name.clone(),
                telemetry_host: tracker_config.telemetry_host.clone(),
                network: network,
                store_behavior: tracker_config.store_behavior.clone(),
            };

            run_telemetry_watcher(specialized).await?;
        }
    }

    // Process candidate verifier configuration.
    let verifier = root_config.candidate_verifier;
    if verifier.enabled {
        let verifier_config = verifier.config.ok_or(anyhow!(
            "No configuration is provided for (enabled) candidate verifier"
        ))?;

        for network_config in verifier_config {
            let network = network_config.network;
            info!(
                "Starting candidate verifier for {} network",
                network.as_ref()
            );

            let specialized = RequirementsProceedingConfig {
                db_uri: root_config.db_uri.clone(),
                db_name: root_config.db_name.clone(),
                rpc_hostname: network_config.rpc_hostname,
                requirements_config: network_config.requirements_config,
                network: network,
            };

            let candidates: Vec<Candidate> = serde_yaml::from_str::<Vec<RawCandidate>>(
                &read_to_string(&network_config.candidate_file)?,
            )?
            .into_iter()
            .map(|raw| Candidate::new(raw.stash, raw.name, network))
            .collect();

            tokio::spawn(async move {
                if let Err(err) = run_requirements_proceeding(specialized, candidates).await {
                    error!("Exiting candidate verifier {:?}", err);
                }
            });
        }
    }

    // Hold it here forever.
    loop {
        time::sleep(Duration::from_secs(60)).await;
    }
}
