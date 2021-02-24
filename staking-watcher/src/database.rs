use crate::{Result, ToBson};
use mongodb::{Client, Collection, Database};

const STAKING_LEDGER_STORE_COLLECTION: &'static str = "staking_ledgers";

pub struct MongoClient {
    db: Database,
}

impl MongoClient {
    pub async fn new(uri: &str, db: &str) -> Result<Self> {
        Ok(MongoClient {
            db: Client::with_uri_str(uri).await?.database(db),
        })
    }
    pub fn get_telemetry_event_store(&self) -> StakingLedgerStore {
        StakingLedgerStore {
            coll: self.db.collection(STAKING_LEDGER_STORE_COLLECTION),
        }
    }
}

pub struct StakingLedgerStore {
    coll: Collection,
}
