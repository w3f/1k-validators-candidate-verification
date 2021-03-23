#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate serde;

use lib::{
    read_candidates, run_requirements_proceeding, run_telemetry_watcher, start_rest_api, Network,
    RequirementsConfig, RequirementsProceedingConfig, RestApiConfig, Result, StoreBehavior,
    TelemetryWatcherConfig, TimetableStoreConfig,
};
use std::fs::read_to_string;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct RootConfig {
    db_uri: String,
    db_name: String,
    services: Vec<ServiceType>,
    rest_api: RestApiConfig,
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
    db_uri: Option<String>,
    db_name: Option<String>,
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

#[actix_web::main]
async fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_module("candidate_verifier", log::LevelFilter::Debug)
        .filter_module("lib", log::LevelFilter::Debug)
        .init();

    info!("Opening config file");
    let config = read_to_string("config/service.yml")
        .or_else(|_| read_to_string("/etc/candidate_verifier/service.yml"))
        .map_err(|err| {
            anyhow!("Failed to open config at 'config/service.yml' or '/etc/candidate_verifier/service.yml': {:?}", err)
        })?;

    let root_config: RootConfig = serde_yaml::from_str(&config)?;

    // Process telemetry tracker configuration.
    for service in root_config.services {
        match service {
            ServiceType::TelemetryWatcher(config) => {
                let specialized = TelemetryWatcherConfig {
                    // Check for custom db configuration or use the global settings.
                    db_uri: config.db_uri.unwrap_or(root_config.db_uri.clone()),
                    db_name: config.db_name.unwrap_or(root_config.db_name.clone()),
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

    start_rest_api(
        root_config.rest_api,
        &root_config.db_uri,
        &root_config.db_name,
    )
    .await
}
