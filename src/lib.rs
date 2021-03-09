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

mod api;
mod database;
mod events;
mod judge;
mod jury;
mod system;

// Re-exports
pub use api::{start_rest_api, RestApiConfig};
pub use database::TimetableStoreConfig;
pub use events::NodeName;
pub use jury::RequirementsConfig;
pub use system::{
    read_candidates, run_requirements_proceeding, run_telemetry_watcher, Candidate, Network,
    RequirementsProceedingConfig, StoreBehavior, TelemetryWatcherConfig,
};

pub type Result<T> = std::result::Result<T, anyhow::Error>;

trait ToBson {
    fn to_bson(&self) -> Result<Bson>;
}

impl<T: Serialize> ToBson for T {
    fn to_bson(&self) -> Result<Bson> {
        Ok(to_bson(self)?)
    }
}

