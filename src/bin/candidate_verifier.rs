#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

use lib::{
    read_candidates, run_requirements_proceeding, run_telemetry_watcher, Network,
    RequirementsConfig, RequirementsProceedingConfig, Result, StoreBehavior,
    TelemetryWatcherConfig, TimetableStoreConfig,
};
use std::fs::read_to_string;
use tokio::time::{self, Duration};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct RootConfig {
    db_uri: String,
    db_name: String,
    services: Vec<ServiceType>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ServiceType {
    TelemetryWatcher(TelemetryTrackerConfig),
    CandidateVerifier(CandidateVerifierConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct TelemetryTrackerConfig {
    telemetry_host: String,
    network: Network,
    store_behavior: RawStoreBehavior,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", content = "config", rename_all = "snake_case")]
enum RawStoreBehavior {
    Store,
    DowntimeCounter(RawCounterConfig),
}

impl RawStoreBehavior {
    fn into_store_behavior(self, network: Network) -> Result<StoreBehavior> {
        Ok(match self {
            RawStoreBehavior::Store => StoreBehavior::Store,
            RawStoreBehavior::DowntimeCounter(config) => StoreBehavior::Counter({
                TimetableStoreConfig {
                    whitelist: read_candidates(&config.candidate_file, network)?
                        .into_iter()
                        .map(|c| c.node_name().clone())
                        .collect(),
                    threshold: config.threshold,
                    max_downtime: config.max_downtime,
                    monitoring_period: config.monitoring_period,
                    is_dummy: false,
                }
            }),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct RawCounterConfig {
    threshold: i64,
    max_downtime: i64,
    monitoring_period: i64,
    candidate_file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct CandidateVerifierConfig {
    rpc_hostname: String,
    network: Network,
    candidate_file: String,
    requirements_config: RequirementsConfig<u128>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_module("candidate_verifier", log::LevelFilter::Debug)
        .filter_module("lib", log::LevelFilter::Debug)
        .init();

    let root_config: RootConfig = serde_yaml::from_str(&read_to_string("config/service.yml")?)?;

    // Process telemetry tracker configuration.
    for service in root_config.services {
        match service {
            ServiceType::TelemetryWatcher(config) => {
                let specialized = TelemetryWatcherConfig {
                    db_uri: root_config.db_uri.clone(),
                    db_name: root_config.db_name.clone(),
                    telemetry_host: config.telemetry_host.clone(),
                    network: config.network,
                    store_behavior: config
                        .store_behavior
                        .clone()
                        .into_store_behavior(config.network)?,
                };

                info!(
                    "Starting telemetry watcher for {} network",
                    config.network.as_ref()
                );

                run_telemetry_watcher(specialized).await?;
            }
            ServiceType::CandidateVerifier(config) => {
                let specialized = RequirementsProceedingConfig {
                    db_uri: root_config.db_uri.clone(),
                    db_name: root_config.db_name.clone(),
                    rpc_hostname: config.rpc_hostname,
                    requirements_config: config.requirements_config,
                    network: config.network,
                };

                let candidates = read_candidates(&config.candidate_file, config.network)?;

                info!(
                    "Starting candidate verifier for {} network",
                    config.network.as_ref()
                );

                tokio::spawn(async move {
                    if let Err(err) = run_requirements_proceeding(specialized, candidates).await {
                        error!("Exiting candidate verifier {:?}", err);
                    }
                });
            }
        }
    }

    // Hold it here forever.
    loop {
        time::sleep(Duration::from_secs(60)).await;
    }
}
