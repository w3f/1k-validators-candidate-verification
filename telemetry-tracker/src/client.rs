use crate::events::{MessageEvent, NodeId, NodeName};
use crate::state::{LogTimestamp, NodeInfo};
use crate::{Result, ToBson};
use mongodb::{Client, Collection, Database};
use tokio_tungstenite::tungstenite::Message;

const TELEMETRY_EVENT_STORE_COLLECTION: &'static str = "telemetry_events";

pub struct MongoClient {
    db: Database,
}

impl MongoClient {
    async fn new(uri: &str, db: &str) -> Result<Self> {
        Ok(MongoClient {
            db: Client::with_uri_str(uri).await?.database(db),
        })
    }
    fn get_telemetry_event_store(&self) -> TelemetryEventStore {
        TelemetryEventStore {
            coll: self.db.collection(TELEMETRY_EVENT_STORE_COLLECTION),
        }
    }
}

pub struct TelemetryEventStore {
    coll: Collection,
}

impl TelemetryEventStore {
    async fn insert_node_info(&self, node_id: &NodeId, node_name: &NodeName) -> Result<()> {
        self.coll
            .update_one(
                doc! {
                    "node_id": node_id.to_bson()?,
                },
                doc! {
                    "$set": {
                        "node_id": node_id.to_bson()?,
                    }
                },
                None,
            )
            .await?;

        Ok(())
    }
    async fn store_event(&self, event: MessageEvent) -> Result<()> {
        unreachable!()
    }
}
