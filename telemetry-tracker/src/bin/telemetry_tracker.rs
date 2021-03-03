#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate anyhow;

use lib::{
    run_requirements_proceeding, run_telemetry_watcher, Network, RequirementsConfig,
    RequirementsProceedingConfig, Result, TelemetryWatcherConfig,
};
use std::fs::read_to_string;
use tokio::time::{self, Duration};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Config {
    db_url: String,
    db_name: String,
    telemetry_tracker: ConfigWrapper<TelemetryTrackerConfig>,
    candidate_verifier: ConfigWrapper<CandidateVerifierConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigWrapper<T> {
    pub enabled: bool,
    #[serde(flatten)]
    pub config: Option<T>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TelemetryTrackerConfig {
    telemetry_host: String,
    networks: Vec<Network>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CandidateVerifierConfig {
    networks: Vec<CandidateVerifierNetworkConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CandidateVerifierNetworkConfig {
    network: Network,
    rpc_hostname: String,
    requirements_config: RequirementsConfig<u128>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let root_config: Config = serde_yaml::from_str(&read_to_string("config/service.yml")?)?;

    // Process telemetry tracker configuration.
    let tracker = root_config.telemetry_tracker;
    if tracker.enabled {
        let tracker_config = tracker.config.ok_or(anyhow!(
            "No configuration is provided for (enabled) telemetry tracker"
        ))?;

        for network in tracker_config.networks {
            let specialized = TelemetryWatcherConfig {
                db_uri: root_config.db_url.clone(),
                db_name: root_config.db_name.clone(),
                telemetry_host: tracker_config.telemetry_host.clone(),
                network: network,
            };

            tokio::spawn(async move {
                if let Err(err) = run_telemetry_watcher(specialized).await {
                    error!("Exiting telemetry watcher: {:?}", err);
                }
            });
        }
    }

    // Process candidate verifier configuration.
    let verifier = root_config.candidate_verifier;
    if verifier.enabled {
        let verifier_config = verifier.config.ok_or(anyhow!(
            "No configuration is provided for (enableD) candidate verifier"
        ))?;

        for network_config in verifier_config.networks {
            let specialized = RequirementsProceedingConfig {
                db_uri: root_config.db_url.clone(),
                db_name: root_config.db_name.clone(),
                rpc_hostname: network_config.rpc_hostname,
                requirements_config: network_config.requirements_config,
                network: network_config.network,
            };

            tokio::spawn(async move {
                /*
                if let Err(err) = run_requirements_proceeding(specialized).await {
                    error!("Exiting candidate verifier {:?}", err);
                }
                */
            });
        }
    }

    // Hold it here forever.
    loop {
        time::sleep(Duration::from_secs(60)).await;
    }
    Ok(())
}
