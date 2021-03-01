use crate::{events::{NodeId, NodeName, NodeVersion, TelemetryEvent}, jury::RequirementsJudgementReport, system::Candidate};
use crate::{Result, ToBson};
use bson::from_document;
use futures::StreamExt;
use mongodb::options::UpdateOptions;
use mongodb::{Client, Collection, Database};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const CANDIDATE_STATE_STORE_COLLECTION: &'static str = "candidate_states";
const TELEMETRY_EVENT_STORE_COLLECTION: &'static str = "telemetry_events";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegisteredStash;
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegisteredController;
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct LogTimestamp(i64);

impl LogTimestamp {
    pub fn new() -> Self {
        LogTimestamp(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs() as i64,
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeActivity {
    pub node_id: NodeId,
    pub node_name: Option<NodeName>,
    pub stash: Option<RegisteredStash>,
    pub controller: Option<RegisteredController>,
    pub last_event_log: LogTimestamp,
    pub event_logs: Vec<EventLog>,
}

impl NodeActivity {
    pub fn new(id: NodeId, name: Option<NodeName>) -> Self {
        NodeActivity {
            node_id: id,
            node_name: name,
            stash: None,
            controller: None,
            last_event_log: LogTimestamp::new(),
            event_logs: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct EventLog {
    pub timestamp: LogTimestamp,
    pub event: TelemetryEvent,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CandidateState {
    pub candidate: Candidate,
    pub last_requirements_report: LogTimestamp,
    pub requirements_report: Vec<RequirementsJudgementReport>,
}

pub struct MongoClient {
    db: Database,
}

impl MongoClient {
    pub async fn new(uri: &str, db: &str) -> Result<Self> {
        Ok(MongoClient {
            db: Client::with_uri_str(uri).await?.database(db),
        })
    }
    pub fn get_candidate_state_store(&self) -> CandidateStateStore {
        CandidateStateStore {
            coll: self.db.collection(CANDIDATE_STATE_STORE_COLLECTION),
        }
    }
    pub fn get_telemetry_event_store(&self) -> TelemetryEventStore {
        TelemetryEventStore {
            coll: self.db.collection(TELEMETRY_EVENT_STORE_COLLECTION),
        }
    }
}

pub struct CandidateStateStore {
    coll: Collection,
}

pub struct TelemetryEventStore {
    coll: Collection,
}

impl TelemetryEventStore {
    #[cfg(test)]
    async fn drop(&self) {
        self.coll.drop(None).await.unwrap();
    }
    async fn insert_node_info(&self, node_id: &NodeId, node_name: Option<&NodeName>) -> Result<()> {
        self.coll
            .update_one(
                doc! {
                    "node_id": node_id.to_bson()?,
                },
                doc! {
                    "$setOnInsert": NodeActivity::new(node_id.clone(), node_name.map(|n| n.clone())).to_bson()?,
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
    pub async fn store_event(&self, event: TelemetryEvent) -> Result<()> {
        let node_id = event.node_id();
        let node_name = event.node_name();

        self.insert_node_info(node_id, node_name).await?;

        self.coll
            .update_one(
                doc! {
                    "node_id": node_id.to_bson()?,
                },
                doc! {
                    "$set": {
                        "last_event_lot": LogTimestamp::new().to_bson()?,
                    },
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
    pub async fn get_node_activity_by_name(&self, name: &NodeName) -> Result<Vec<NodeActivity>> {
        let mut cursor = self
            .coll
            .find(
                doc! {
                    "node_name": name.to_bson()?,
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
    pub async fn get_node_activity_by_id(&self, node_id: &NodeId) -> Result<Option<NodeActivity>> {
        if let Some(doc) = self
            .coll
            .find_one(
                doc! {
                    "node_id": node_id.to_bson()?,
                },
                None,
            )
            .await?
        {
            Ok(Some(from_document(doc)?))
        } else {
            Ok(None)
        }
    }
    pub async fn get_majority_client_version(&self) -> Result<Option<NodeVersion>> {
        let mut cursor = self
            .coll
            .find(
                doc! {
                    "event_logs.event.type": "added_node",
                    "event_logs.event.content.details.version": {
                        "$exists": true,
                    }
                },
                None,
            )
            .await?;

        let mut observed_versions = HashMap::new();
        while let Some(doc) = cursor.next().await {
            let node_activity: NodeActivity = from_document(doc?)?;

            let mut version = None;
            for log in node_activity.event_logs {
                match log.event {
                    TelemetryEvent::AddedNode(event) => {
                        version = Some(event.details.version);
                        break;
                    }
                    _ => {}
                }
            }

            // Ignore if the version of the node could not be determined.
            if let Some(version) = version {
                observed_versions
                    .entry(version)
                    .and_modify(|occurences| *occurences += 1)
                    .or_insert(1);
            }
        }

        // Find the client version with the most occurrences.
        let mut m_version = None;
        let mut m_occurrences = 0;
        for (version, occurrences) in observed_versions {
            if occurrences > m_occurrences {
                m_version = Some(version);
                m_occurrences = occurrences;
            }
        }

        Ok(m_version)
    }
    pub async fn get_observed_names(&self) -> Result<Vec<NodeName>> {
        let mut cursor = self
            .coll
            .find(
                doc! {
                    "node_name": {
                        "$exists": true,
                    },
                    "node_name": 1
                },
                None,
            )
            .await?;

        let mut names = vec![];
        while let Some(doc) = cursor.next().await {
            names.push(from_document(doc?)?);
        }

        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{AddedNodeEvent, NodeDetails, NodeId, NodeImplementation, TelemetryEvent};

    #[tokio::test]
    async fn store_event() {
        // Create client.
        let client = MongoClient::new("mongodb://localhost:27017/", "test_store_event")
            .await
            .unwrap()
            .get_telemetry_event_store();

        client.drop().await;

        // Prepare events.
        let node_1 = NodeId::from(1);
        let node_2 = NodeId::from(2);
        let node_3 = NodeId::from(3);

        let node_1_events = [
            TelemetryEvent::TestMessage(node_1.clone(), "Event A".to_string()),
            TelemetryEvent::TestMessage(node_1.clone(), "Event B".to_string()),
        ];

        let node_2_events = [TelemetryEvent::TestMessage(
            node_2.clone(),
            "Event C".to_string(),
        )];

        let node_3_events = [
            TelemetryEvent::TestMessage(node_3.clone(), "Event D".to_string()),
            TelemetryEvent::TestMessage(node_3.clone(), "Event E".to_string()),
        ];

        // Store all events.
        for event in node_1_events
            .iter()
            .chain(node_2_events.iter())
            .chain(node_3_events.iter())
        {
            client.store_event(event.clone()).await.unwrap();
        }

        // Check NodeId 1.
        let stored = client
            .get_node_activity_by_id(&node_1)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.event_logs.len(), node_1_events.len());
        for (log, expected) in stored.event_logs.iter().zip(node_1_events.iter()) {
            assert_eq!(&log.event, expected);
        }

        // Check NodeId 2.
        let stored = client
            .get_node_activity_by_id(&node_2)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.event_logs.len(), node_2_events.len());
        for (log, expected) in stored.event_logs.iter().zip(node_2_events.iter()) {
            assert_eq!(&log.event, expected);
        }

        // Check NodeId 3.
        let stored = client
            .get_node_activity_by_id(&node_3)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.event_logs.len(), node_3_events.len());
        for (log, expected) in stored.event_logs.iter().zip(node_3_events.iter()) {
            assert_eq!(&log.event, expected);
        }

        //Add new events to NodeId 1.
        let node_1_events_new = [
            TelemetryEvent::TestMessage(node_1.clone(), "Event F".to_string()),
            TelemetryEvent::TestMessage(node_1.clone(), "Event G".to_string()),
        ];

        for event in node_1_events_new.iter() {
            client.store_event(event.clone()).await.unwrap();
        }

        // Check NodeId 1.
        let stored = client
            .get_node_activity_by_id(&node_1)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            stored.event_logs.len(),
            node_1_events.len() + node_1_events_new.len()
        );

        for (log, expected) in stored
            .event_logs
            .iter()
            .zip(node_1_events.iter().chain(node_1_events_new.iter()))
        {
            assert_eq!(&log.event, expected);
        }

        client.drop().await;
    }

    #[tokio::test]
    async fn get_majority_client_version() {
        // Create client.
        let client = MongoClient::new(
            "mongodb://localhost:27017/",
            "test_get_majority_client_version",
        )
        .await
        .unwrap()
        .get_telemetry_event_store();

        client.drop().await;

        let messages = [
            TelemetryEvent::AddedNode({
                let mut event = AddedNodeEvent::alice();
                event.details.version = NodeVersion::from("2.0".to_string());
                event
            }),
            TelemetryEvent::AddedNode({
                let mut event = AddedNodeEvent::alice();
                event.details.version = NodeVersion::from("1.0".to_string());
                event
            }),
            TelemetryEvent::AddedNode({
                let mut event = AddedNodeEvent::alice();
                event.details.version = NodeVersion::from("2.0".to_string());
                event
            }),
        ];

        for message in &messages {
            client.store_event(message.clone()).await.unwrap();
        }

        let version = client.get_majority_client_version().await.unwrap().unwrap();
        assert_eq!(version, NodeVersion::from("2.0".to_string()));

        client.drop().await;
    }
}
