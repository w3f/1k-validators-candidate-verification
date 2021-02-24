use crate::events::{MessageEvent, NodeId, NodeName};
use crate::state::{EventLog, LogTimestamp, NodeInfo};
use crate::{Result, ToBson};
use mongodb::options::UpdateOptions;
use mongodb::{Client, Collection, Database};
use tokio_tungstenite::tungstenite::Message;

const TELEMETRY_EVENT_STORE_COLLECTION: &'static str = "telemetry_events";

pub struct MongoClient {
    db: Database,
}

impl MongoClient {
    pub async fn new(uri: &str, db: &str) -> Result<Self> {
        Ok(MongoClient {
            db: Client::with_uri_str(uri).await?.database(db),
        })
    }
    pub fn get_telemetry_event_store(&self) -> TelemetryEventStore {
        TelemetryEventStore {
            coll: self.db.collection(TELEMETRY_EVENT_STORE_COLLECTION),
        }
    }
}

pub struct TelemetryEventStore {
    coll: Collection,
}

impl TelemetryEventStore {
    async fn insert_node_info(&self, node_id: &NodeId) -> Result<()> {
        self.coll
            .update_one(
                doc! {
                    "node_id": node_id.to_bson()?,
                },
                doc! {
                    "$set": {
                        "last_event_log": LogTimestamp::new().to_bson()?,
                    },
                    "$setOnInsert": NodeInfo::from_node_id(node_id.clone()).to_bson()?,
                },
                Some({
                    let mut options = UpdateOptions::default();
                    options.upsert = Some(true);
                    options
                }),
            )
            .await?;

        Ok(())
    }
    pub async fn store_event(&self, event: MessageEvent) -> Result<()> {
        let node_id = event.node_id();
        self.insert_node_info(node_id).await?;

        self.coll
            .update_one(
                doc! {
                    "node_id": node_id.to_bson()?,
                },
                doc! {
                    "$push": {
                        "event_logs": EventLog {
                            timestamp: LogTimestamp::new(),
                            event: event,
                        }.to_bson()?
                    }
                },
                None,
            )
            .await?;

        Ok(())
    }
}
