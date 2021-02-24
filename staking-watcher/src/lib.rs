use bson::ser::to_bson;
use bson::Bson;
use serde::Serialize;

mod chaindata;
mod database;

type Result<T> = std::result::Result<T, anyhow::Error>;

trait ToBson {
    fn to_bson(&self) -> Result<Bson>;
}

impl<T: Serialize> ToBson for T {
    fn to_bson(&self) -> Result<Bson> {
        Ok(to_bson(self)?)
    }
}
