#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate bson;

use bson::ser::to_bson;
use bson::Bson;
use serde::ser::Serialize;

mod database;
mod events;
mod judge;
mod jury;
mod system;

// Re-exports
pub use system::{
    run_requirements_proceeding, run_telemetry_watcher, RequirementsProceedingConfig,
    TelemetryWatcherConfig,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigWrapper<T> {
    pub enabled: bool,
    #[serde(flatten)]
    pub config: Option<T>,
}

pub type Result<T> = std::result::Result<T, anyhow::Error>;

trait ToBson {
    fn to_bson(&self) -> Result<Bson>;
}

impl<T: Serialize> ToBson for T {
    fn to_bson(&self) -> Result<Bson> {
        Ok(to_bson(self)?)
    }
}
