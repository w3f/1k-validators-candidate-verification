use crate::events::{MessageEvent, NodeId, NodeName};
use crate::state::{EventLog, LogTimestamp, NodeInfo};
use crate::{Result, ToBson};
use bson::{from_document, Document};
use futures::{StreamExt, TryStreamExt};
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
    async fn insert_node_info(&self, node_id: &NodeId, node_name: Option<&NodeName>) -> Result<()> {
        println!("HERE");
        self.coll
            .update_one(
                doc! {
                    "node_id": node_id.to_bson()?,
                },
                doc! {
                    "$setOnInsert": NodeInfo::new(node_id.clone(), node_name.map(|n| n.clone())).to_bson()?,
                },
                Some({
                    let mut options = UpdateOptions::default();
                    options.upsert = Some(true);
                    options
                }),
            )
            .await?;
        println!("DONE");

        Ok(())
    }
    pub async fn store_event(&self, event: MessageEvent) -> Result<()> {
        let node_id = event.node_id();
        let node_name = event.node_name();

        self.insert_node_info(node_id, node_name).await?;

        self.coll
            .update_one(
                doc! {
                    "node_id": node_id.to_bson()?,
                },
                doc! {
                    "last_event_lot": LogTimestamp::new().to_bson()?,
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
    pub async fn get_info_by_name(&self, name: &NodeName) -> Result<Vec<NodeInfo>> {
        let mut cursor = self
            .coll
            .find(
                doc! {
                    "name": name.to_bson()?,
                },
                None,
            )
            .await?;

        let mut entries = vec![];
        while let Some(doc) = cursor.next().await {
            entries.push(from_document(doc?)?);
        }

        Ok(entries)
    }
    pub async fn get_info_by_id(&self, node_id: &NodeId) -> Result<Vec<NodeInfo>> {
        let mut cursor = self
            .coll
            .find(
                doc! {
                    "id": node_id.to_bson()?,
                },
                None,
            )
            .await?;

        let mut entries = vec![];
        while let Some(doc) = cursor.next().await {
            entries.push(from_document(doc?)?);
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{MessageEvent, NodeId};

    #[tokio::test]
    async fn store_event() {
        let client = MongoClient::new("mongodb://localhost:27017/", "test_db")
            .await
            .unwrap()
            .get_telemetry_event_store();

        let events = [
            MessageEvent::TestMessage(NodeId::from(1), "Event A".to_string()),
            MessageEvent::TestMessage(NodeId::from(1), "Event B".to_string()),
            MessageEvent::TestMessage(NodeId::from(2), "Event C".to_string()),
            MessageEvent::TestMessage(NodeId::from(3), "Event D".to_string()),
            MessageEvent::TestMessage(NodeId::from(3), "Event E".to_string()),
        ];

        for event in events.iter() {
            client.store_event(event.clone()).await.unwrap();
        }

        let stored = client.get_info_by_id(&NodeId::from(1)).await.unwrap();
        assert_eq!(stored.len(), 2);

    }
}
