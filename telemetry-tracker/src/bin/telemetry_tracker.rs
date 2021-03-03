#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate anyhow;

use lib::{
    run_requirements_proceeding, run_telemetry_watcher, ConfigWrapper,
    RequirementsProceedingConfig, Result, TelemetryWatcherConfig,
};
use std::fs::read_to_string;
use tokio::time::{self, Duration};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Config {
    telemetry_tracker: ConfigWrapper<TelemetryWatcherConfig>,
    candidate_verifier: ConfigWrapper<RequirementsProceedingConfig>,
}

#[tokio::main]
async fn main() -> Result<()> {
    /*
    let config: Config = serde_yaml::from_str(&read_to_string("config/service.yml")?)?;

    // Process telemetry tracker configuration.
    let tracker = config.telemetry_tracker;
    if tracker.enabled {
        let config = tracker.config.ok_or(anyhow!(
            "No configuration is provided for (enabled) telemetry tracker"
        ))?;

        tokio::spawn(async move {
            if let Err(err) = run_telemetry_watcher(config).await {
                error!("Exiting telemetry watcher: {:?}", err);
            }
        });
    }

    // Process candidate verifier configuration.
    let verifier = config.candidate_verifier;
    if verifier.enabled {
        let config = verifier.config.ok_or(anyhow!(
            "No configuration is provided for (enableD) candidate verifier"
        ))?;

        tokio::spawn(async move {
            if let Err(err) = run_requirements_proceeding(config).await {
                error!("Exiting candidate verifier {:?}", err);
            }
        });
    }

    // Hold it here forever.
    loop {
        time::sleep(Duration::from_secs(60)).await;
    }
    */
    Ok(())
}
