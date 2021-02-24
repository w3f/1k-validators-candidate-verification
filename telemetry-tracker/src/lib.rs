#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate bson;

use bson::Bson;
use std::convert::TryInto;

mod client;
mod events;
mod state;
mod system;

const DEFAULT_TELEMETRY: &'static str = "wss://telemetry-backend.w3f.community/feed";

type Result<T> = std::result::Result<T, anyhow::Error>;

trait ToBson {
    fn to_bson(&self) -> Result<Bson>;
}

impl<T: serde::Serialize> ToBson for T {
    fn to_bson(&self) -> Result<Bson> {
        Ok(serde_json::to_value(self)?.try_into()?)
    }
}
