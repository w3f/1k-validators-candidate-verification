#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate bson;

use bson::de::from_document;
use bson::ser::to_bson;
use bson::{Bson, Document};
use serde::de::{Deserialize, DeserializeOwned};
use serde::ser::Serialize;
use std::convert::TryInto;

mod database;
mod events;
mod state;
mod system;

const DEFAULT_TELEMETRY: &'static str = "wss://telemetry-backend.w3f.community/feed";

type Result<T> = std::result::Result<T, anyhow::Error>;

trait ToBson {
    fn to_bson(&self) -> Result<Bson>;
}

impl<T: Serialize> ToBson for T {
    fn to_bson(&self) -> Result<Bson> {
        Ok(to_bson(self)?)
    }
}
