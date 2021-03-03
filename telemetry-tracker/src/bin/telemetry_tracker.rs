#[macro_use]
extern crate serde;
#[macro_use]
extern crate anyhow;

use lib::{
    run_requirements_proceeding, run_telemetry_watcher, ConfigWrapper,
    RequirementsProceedingConfig, Result, TelemetryWatcherConfig,
};
use std::fs::read_to_string;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Config {
    telemetry_tracker: ConfigWrapper<TelemetryWatcherConfig>,
    candidate_validator: ConfigWrapper<RequirementsProceedingConfig>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config: Config = serde_yaml::from_str(&read_to_string("config/service.yml")?)?;

    // Process telemetry tracker configuration.
    let tracker = config.telemetry_tracker;
    if tracker.enabled {
        run_telemetry_watcher(tracker.config.ok_or(anyhow!(
            "No configuration is provided for (enabled) telemetry tracker"
        ))?)
        .await?;
    }

    // Process candidate validator configuration.
    let validator = config.candidate_validator;
    if validator.enabled {
        run_requirements_proceeding(validator.config.ok_or(anyhow!(
            "No configuration is provided for (enableD) candidate validator"
        ))?)
        .await?;
    }

    Ok(())
}
