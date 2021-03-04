use crate::{
    events::{NodeId, NodeName, NodeVersion, TelemetryEvent},
    jury::RequirementsJudgementReport,
    system::{Candidate, Network},
};
use crate::{Result, ToBson};
use bson::{from_document, Document};
use futures::StreamExt;
use mongodb::options::UpdateOptions;
use mongodb::{Client, Collection, Database};
use std::collections::{HashMap, HashSet};
use std::ops::{Add, Sub};
use std::time::{SystemTime, UNIX_EPOCH};

const CANDIDATE_STATE_STORE_COLLECTION: &'static str = "candidate_states";
const TELEMETRY_EVENT_STORE_COLLECTION: &'static str = "telemetry_events";
const TIMETABLE_STORE_COLLECTION: &'static str = "time_tables";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegisteredStash;
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegisteredController;
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
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
    pub fn as_secs(&self) -> i64 {
        self.0
    }
}

impl Add for LogTimestamp {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        LogTimestamp(self.0 + other.0)
    }
}

impl Sub for LogTimestamp {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        LogTimestamp(self.0 - other.0)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeActivity {
    pub node_id: NodeId,
    pub node_name: Option<NodeName>,
    pub stash: Option<RegisteredStash>,
    pub controller: Option<RegisteredController>,
    #[serde(skip_serializing)]
    pub last_event_timestamp: Option<LogTimestamp>,
    #[serde(skip_serializing)]
    pub events: Vec<EventLog<TelemetryEvent>>,
}

impl NodeActivity {
    pub fn new(id: NodeId, name: Option<NodeName>) -> Self {
        NodeActivity {
            node_id: id,
            node_name: name,
            stash: None,
            controller: None,
            last_event_timestamp: None,
            events: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct EventLog<T> {
    pub timestamp: LogTimestamp,
    pub event: T,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CandidateState {
    pub candidate: Candidate,
    pub last_report_timestamp: Option<LogTimestamp>,
    pub judgement_reports: Vec<EventLog<RequirementsJudgementReport>>,
}

impl CandidateState {
    pub fn new(candidate: Candidate) -> Self {
        CandidateState {
            candidate: candidate,
            last_report_timestamp: None,
            judgement_reports: vec![],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Timetable {
    node_name: NodeName,
    last_event: LogTimestamp,
    offline_counter: i64,
    checkpoint: Option<LogTimestamp>,
    start_period: Option<LogTimestamp>,
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
    pub fn get_candidate_state_store(&self, network: &Network) -> CandidateStateStore {
        CandidateStateStore {
            coll: self.db.collection(&format!(
                "{}_{}",
                CANDIDATE_STATE_STORE_COLLECTION,
                network.as_ref()
            )),
        }
    }
    pub fn get_telemetry_event_store(&self, network: &Network) -> TelemetryEventStore {
        TelemetryEventStore {
            coll: self.db.collection(&format!(
                "{}_{}",
                TELEMETRY_EVENT_STORE_COLLECTION,
                network.as_ref()
            )),
        }
    }
    pub fn get_time_table_store(
        &self,
        config: TimetableStoreConfig,
        network: &Network,
    ) -> TimetableStore {
        TimetableStore {
            coll: self.db.collection(&format!(
                "{}_{}",
                TIMETABLE_STORE_COLLECTION,
                network.as_ref()
            )),
            name_lookup: HashMap::new(),
            config: config,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CandidateStateStore {
    coll: Collection,
}

impl CandidateStateStore {
    pub async fn store_requirements_report(
        &self,
        candidate: &Candidate,
        report: RequirementsJudgementReport,
    ) -> Result<()> {
        self.coll
            .update_one(
                doc! {
                    "candidate": candidate.to_bson()?,
                },
                doc! {
                    "$set": {
                        "last_report_timestamp": LogTimestamp::new().to_bson()?,
                    },
                    "$push": {
                        "judgement_reports": EventLog {
                            timestamp: LogTimestamp::new(),
                            event: report,
                        }.to_bson()?,
                    }
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
    pub async fn fetch_candidate_state(
        &self,
        candidate: &Candidate,
    ) -> Result<Option<CandidateState>> {
        let doc = self
            .coll
            .find_one(
                doc! {
                    "candidate": candidate.to_bson()?,
                },
                None,
            )
            .await?;

        if let Some(doc) = doc {
            Ok(Some(from_document(doc)?))
        } else {
            Ok(None)
        }
    }
}

pub struct TimetableStoreConfig {
    whitelist: HashSet<NodeName>,
    threshold: i64,
    max_downtime: i64,
    monitoring_period: i64,
}

pub struct TimetableStore {
    coll: Collection,
    name_lookup: HashMap<NodeId, NodeName>,
    config: TimetableStoreConfig,
}

impl TimetableStore {
    pub async fn track_event(
        &mut self,
        event: TelemetryEvent,
        now: Option<LogTimestamp>,
    ) -> Result<()> {
        let now = now.unwrap_or(LogTimestamp::new());
        let node_id = event.node_id();

        let node_name = match event {
            // `AddedNode` events need special treatment since only those
            // specify a node name.
            TelemetryEvent::AddedNode(ref event) => {
                let node_name = &event.details.name;

                // Only process whitelisted node names.
                if !self.config.whitelist.contains(&node_name) {
                    return Ok(());
                }

                // Find existing node name duplicates which will be pruned. This
                // might be slightly time consuming, but `AddedNode` events are
                // primarily generated on initial connection and rarely occur
                // later on.
                let mut to_delete = vec![];
                for (curr_node_id, curr_node_name) in &self.name_lookup {
                    if curr_node_name == node_name {
                        to_delete.push(curr_node_id.clone());
                    }
                }

                for node_id in &to_delete {
                    self.name_lookup.remove(node_id);
                }

                // Update the node Id with the newest, corresponding node name.
                self.name_lookup.insert(node_id.clone(), node_name.clone());

                node_name
            }
            _ => {
                // Lookup node name or whether to ignore the event.
                if let Some(node_name) = self.name_lookup.get(node_id) {
                    node_name
                } else {
                    return Ok(());
                }
            }
        };

        // Update last event timestamp.
        self.coll
            .update_one(
                doc! {
                    "node_name": node_name.to_bson()?,
                },
                doc! {
                    "$set": {
                        "last_event": now.to_bson()?,
                        "checkpoint": null,
                    },
                    "$setOnInsert": {
                        "start_period": now.to_bson()?,
                        "offline_counter": 0,
                    }
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
    pub async fn process_time_tables(&self, now: Option<LogTimestamp>) -> Result<()> {
        let now = now.unwrap_or(LogTimestamp::new());
        let threshold = now.as_secs() - self.config.threshold;

        let mut cursor = self
            .coll
            .find(
                doc! {
                    "last_event": {
                        "$lt": threshold.to_bson()?,
                    }
                },
                None,
            )
            .await?;

        while let Some(doc) = cursor.next().await {
            let timetable: Timetable = from_document(doc?)?;

            println!(">>> {:?}", timetable);
            // Update offline counters and checkpoints.
            let mut update = if let Some(checkpoint) = timetable.checkpoint {
                println!("GOT HERE!!!!!!");
                doc! {
                    "$set": {
                        "checkpoint": now.to_bson()?,
                    },
                    "$inc": {
                        "offline_counter": (now - checkpoint).to_bson()?,
                    }
                }
            } else {
                println!("NOW: {}, LAST EVENT: {}", now.as_secs(), timetable.last_event.as_secs());
                doc! {
                    "$set": {
                        "checkpoint": now.to_bson()?,
                        "start_period": now.to_bson()?,
                    },
                    "$inc": {
                        "offline_counter": (now - timetable.last_event).to_bson()?,
                    }
                }
            };

            // Check if the monitoring period has completed and reset, if appropriate.
            if let Some(start_period) = timetable.start_period {
                if start_period.as_secs() < now.as_secs() - self.config.monitoring_period {
                    update.extend(doc! {
                        "$set": {
                            "offline_counter": 0,
                            "start_period": now.to_bson()?,
                        }
                    });
                }
            }

            // Insert state into storage.
            self.coll
                .update_one(
                    doc! {
                        "node_name": timetable.node_name.to_bson()?,
                    },
                    update,
                    None,
                )
                .await?;
        }

        Ok(())
    }
    /// Checks whether the candidate has any downtime. Returns a tuple (if the
    /// candidate could be found) where the `bool` indicates whether the
    /// candidate should be punished (exceeds the maximum downtime) and the
    /// `i64` indicates the downtime in seconds.
    pub async fn has_downtime_violation(
        &self,
        candidate: &Candidate,
    ) -> Result<Option<(bool, i64)>> {
        if let Some(doc) = self
            .coll
            .find_one(
                doc! {
                    "node_name": candidate.node_name().to_bson()?,
                },
                None,
            )
            .await?
        {
            let offline_counter = from_document::<Timetable>(doc)?.offline_counter;
            if offline_counter > self.config.max_downtime {
                Ok(Some((true, offline_counter)))
            } else {
                Ok(Some((false, offline_counter)))
            }
        } else {
            Ok(None)
        }
    }
    #[cfg(test)]
    async fn drop(&self) {
        self.coll.drop(None).await.unwrap();
    }
}

#[derive(Debug, Clone)]
pub struct TelemetryEventStore {
    coll: Collection,
}

impl TelemetryEventStore {
    #[cfg(test)]
    async fn drop(&self) {
        self.coll.drop(None).await.unwrap();
    }
    pub async fn store_event(&self, event: TelemetryEvent) -> Result<()> {
        self.store_event_with_timestamp(event, None).await
    }
    /// Private method to set timestamp manually. Required by certain tests.
    async fn store_event_with_timestamp(
        &self,
        event: TelemetryEvent,
        timestamp: Option<LogTimestamp>,
    ) -> Result<()> {
        let node_id = event.node_id();
        let node_name = event.node_name();
        let timestamp = timestamp.unwrap_or(LogTimestamp::new());

        self.coll
            .update_one(
                doc! {
                    "node_id": node_id.to_bson()?,
                },
                doc! {
                    "$set": {
                        "last_event_timestamp": timestamp.to_bson()?,
                    },
                    "$push": {
                        "events": EventLog {
                            timestamp: timestamp,
                            event: event,
                        }.to_bson()?
                    }
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
    pub async fn get_node_ids_by_name(&self, name: &NodeName) -> Result<Vec<NodeId>> {
        let mut cursor = self
            .coll
            .find(
                doc! {
                    "node_name": name.to_bson()?,
                    "_id": 0,
                    "node_id":  1,
                },
                None,
            )
            .await?;

        let mut node_ids: Vec<NodeId> = vec![];
        while let Some(doc) = cursor.next().await {
            node_ids.push(from_document(doc?)?);
        }

        Ok(node_ids)
    }
    // TODO: Required?
    #[allow(unused)]
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
    // TODO: Make use of this.
    #[allow(unused)]
    pub async fn get_majority_client_version(&self) -> Result<Option<NodeVersion>> {
        let mut cursor = self
            .coll
            .find(
                doc! {
                    "events.event.type": "added_node",
                    "events.event.content.details.version": {
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
            for log in node_activity.events {
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
    // TODO: Required?
    #[allow(unused)]
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
    pub async fn verify_node_uptime(
        &self,
        node_id: &NodeId,
        last: u64,
        max_diff: u64,
    ) -> Result<bool> {
        let events_after = LogTimestamp::new().as_secs() - last as i64;

        let cursor = self
            .coll
            .aggregate(
                vec![
                    doc! {
                        "$match": {
                            "events.timestamp": {
                                "$gt": events_after.to_bson()?,
                            },
                            "events.event.content.node_id": node_id.to_bson()?,
                        }
                    },
                    doc! {
                        "$addFields": {
                            "min": {
                                "$min": "$events.timestamp",
                            },
                            "max": {
                                "$max": "$events.timestamp",
                            },
                            "total": {
                                "$size": "$events",
                            }
                        }
                    },
                    doc! {
                        "$project": {
                            "_id": 0,
                            "min": 1,
                            "max": 1,
                            "total": 1,
                        }
                    },
                ],
                None,
            )
            .await?;

        // Process BSON results
        let mut docs = cursor
            .collect::<Vec<std::result::Result<Document, _>>>()
            .await;

        if docs.len() == 0 || docs.len() > 1 {
            return Err(anyhow!("invalid query result"));
        }

        let doc = docs.remove(0).map_err(|_err| anyhow!(""))?;

        #[derive(Deserialize, Serialize)]
        struct QueryResult {
            min: i64,
            max: i64,
            total: i64,
        }

        // Verify uptime by checking whether the average gap sizes between
        // events are below the specified time (`max_diff`).
        let res: QueryResult = from_document(doc)?;

        if (res.max - res.min) / (res.total.saturating_sub(1)) <= max_diff as i64 {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{AddedNodeEvent, HardwareEvent, NodeId, NodeStatsEvent, TelemetryEvent};
    use crate::system::Network;

    #[tokio::test]
    async fn store_event() {
        // Create client.
        let client = MongoClient::new("mongodb://localhost:27017/", "test_store_event")
            .await
            .unwrap()
            .get_telemetry_event_store(&Network::Polkadot);

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

        assert_eq!(stored.events.len(), node_1_events.len());
        for (log, expected) in stored.events.iter().zip(node_1_events.iter()) {
            assert_eq!(&log.event, expected);
        }

        // Check NodeId 2.
        let stored = client
            .get_node_activity_by_id(&node_2)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.events.len(), node_2_events.len());
        for (log, expected) in stored.events.iter().zip(node_2_events.iter()) {
            assert_eq!(&log.event, expected);
        }

        // Check NodeId 3.
        let stored = client
            .get_node_activity_by_id(&node_3)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.events.len(), node_3_events.len());
        for (log, expected) in stored.events.iter().zip(node_3_events.iter()) {
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
            stored.events.len(),
            node_1_events.len() + node_1_events_new.len()
        );

        for (log, expected) in stored
            .events
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
        .get_telemetry_event_store(&Network::Polkadot);

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

    #[tokio::test]
    async fn verify_node_uptime_valid() {
        // Create client.
        let client = MongoClient::new(
            "mongodb://localhost:27017/",
            "test_verify_node_uptime_valid",
        )
        .await
        .unwrap()
        .get_telemetry_event_store(&Network::Polkadot);

        client.drop().await;

        let messages = [
            (TelemetryEvent::AddedNode(AddedNodeEvent::alice()), 0),
            (TelemetryEvent::Hardware(HardwareEvent::alice()), 10),
            (TelemetryEvent::NodeStats(NodeStatsEvent::alice()), 20),
            (TelemetryEvent::AddedNode(AddedNodeEvent::alice()), 30),
            (TelemetryEvent::AddedNode(AddedNodeEvent::alice()), 40),
        ];

        let starting = LogTimestamp::new().as_secs();
        for (message, interval) in &messages {
            client
                .store_event_with_timestamp(
                    message.clone(),
                    Some(LogTimestamp(starting + interval)),
                )
                .await
                .unwrap();
        }

        let is_online = client
            .verify_node_uptime(&NodeId::alice(), 1_000, 10)
            .await
            .unwrap();

        assert!(is_online);
        client.drop().await;
    }

    #[tokio::test]
    async fn verify_node_uptime_invalid() {
        // Create client.
        let client = MongoClient::new(
            "mongodb://localhost:27017/",
            "test_verify_node_uptime_invalid",
        )
        .await
        .unwrap()
        .get_telemetry_event_store(&Network::Polkadot);

        client.drop().await;

        let messages = [
            (TelemetryEvent::AddedNode(AddedNodeEvent::alice()), 0),
            (TelemetryEvent::Hardware(HardwareEvent::alice()), 10),
            (TelemetryEvent::NodeStats(NodeStatsEvent::alice()), 40),
            (TelemetryEvent::AddedNode(AddedNodeEvent::alice()), 50),
            (TelemetryEvent::AddedNode(AddedNodeEvent::alice()), 60),
        ];

        let starting = LogTimestamp::new().as_secs();
        for (message, interval) in &messages {
            client
                .store_event_with_timestamp(
                    message.clone(),
                    Some(LogTimestamp(starting + interval)),
                )
                .await
                .unwrap();
        }

        let is_online = client
            .verify_node_uptime(&NodeId::alice(), 1_000, 10)
            .await
            .unwrap();

        assert!(!is_online);

        client.drop().await;
    }

    #[tokio::test]
    async fn track_event_only_uptime() {
        let config = TimetableStoreConfig {
            whitelist: vec![NodeName::alice(), NodeName::bob()]
                .into_iter()
                .collect(),
            threshold: 12,
            max_downtime: 50,
            monitoring_period: 100,
        };

        // Create client.
        let mut client =
            MongoClient::new("mongodb://localhost:27017/", "test_track_event_only_uptime")
                .await
                .unwrap()
                .get_time_table_store(config, &Network::Polkadot);

        client.drop().await;

        let alice = Candidate::alice();
        let bob = Candidate::bob();
        let eve = Candidate::eve();

        // Ignored events (node name not found).
        let events = [
            (TelemetryEvent::NodeStats(NodeStatsEvent::alice()), 0),
            (TelemetryEvent::NodeStats(NodeStatsEvent::bob()), 0),
            (TelemetryEvent::NodeStats(NodeStatsEvent::eve()), 0),
        ];

        let starting = LogTimestamp::new().as_secs();
        for (event, interval) in &events {
            client
                .track_event(event.clone(), Some(LogTimestamp(starting + interval)))
                .await
                .unwrap();

            client.process_time_tables(Some(LogTimestamp(starting + interval))).await.unwrap();
        }

        // Candidates could not be found (`AddedNode` events must be tracked first).
        assert!(client.has_downtime_violation(&alice).await.unwrap().is_none());
        assert!(client.has_downtime_violation(&bob).await.unwrap().is_none());
        assert!(client.has_downtime_violation(&eve).await.unwrap().is_none());

        // Valid events (node name found)
        let events = [
            // Alice
            (TelemetryEvent::AddedNode(AddedNodeEvent::alice()), 0),
            (TelemetryEvent::NodeStats(NodeStatsEvent::alice()), 10),
            (TelemetryEvent::NodeStats(NodeStatsEvent::alice()), 20),
            // Bob
            (TelemetryEvent::AddedNode(AddedNodeEvent::bob()), 0),
            (TelemetryEvent::NodeStats(NodeStatsEvent::bob()), 10),
            (TelemetryEvent::NodeStats(NodeStatsEvent::bob()), 20),
            // Eve
            (TelemetryEvent::AddedNode(AddedNodeEvent::eve()), 0),
            (TelemetryEvent::NodeStats(NodeStatsEvent::eve()), 10),
            (TelemetryEvent::NodeStats(NodeStatsEvent::eve()), 20),
        ];

        for (event, interval) in &events {
            client
                .track_event(event.clone(), Some(LogTimestamp(starting + interval)))
                .await
                .unwrap();

            client.process_time_tables(Some(LogTimestamp(starting + interval))).await.unwrap();
        }

        let (punish, downtime) = client.has_downtime_violation(&alice).await.unwrap().unwrap();
        assert_eq!(punish, false);
        assert_eq!(downtime, 0);

        let (punish, downtime) = client.has_downtime_violation(&bob).await.unwrap().unwrap();
        assert_eq!(punish, false);
        assert_eq!(downtime, 0);

        // Eve is not on the whitelist.
        assert!(client.has_downtime_violation(&eve).await.unwrap().is_none());

        client.drop().await;
    }

    #[tokio::test]
    async fn track_event_downtime() {
        let config = TimetableStoreConfig {
            whitelist: vec![NodeName::alice(), NodeName::bob()]
                .into_iter()
                .collect(),
            threshold: 12,
            max_downtime: 50,
            monitoring_period: 100,
        };

        // Create client.
        let mut client =
            MongoClient::new("mongodb://localhost:27017/", "track_event_downtime")
                .await
                .unwrap()
                .get_time_table_store(config, &Network::Polkadot);

        client.drop().await;

        let alice = Candidate::alice();
        let bob = Candidate::bob();

        // Valid events (node name found)
        let events = [
            // Alice
            (TelemetryEvent::AddedNode(AddedNodeEvent::alice()), 0),
            (TelemetryEvent::NodeStats(NodeStatsEvent::alice()), 10),
            (TelemetryEvent::NodeStats(NodeStatsEvent::alice()), 20),
            (TelemetryEvent::AddedNode(AddedNodeEvent::alice()), 30),
            (TelemetryEvent::NodeStats(NodeStatsEvent::alice()), 40),
            (TelemetryEvent::NodeStats(NodeStatsEvent::alice()), 50),
            // Bob (downtimes, exceeds thresholds).
            (TelemetryEvent::AddedNode(AddedNodeEvent::bob()), 0),
            (TelemetryEvent::NodeStats(NodeStatsEvent::bob()), 20), // Downtime: 20 secs.
            (TelemetryEvent::NodeStats(NodeStatsEvent::bob()), 30),
        ];

        let starting = LogTimestamp::new().as_secs();
        for (event, interval) in &events {
            client
                .track_event(event.clone(), Some(LogTimestamp(starting + interval)))
                .await
                .unwrap();

            client.process_time_tables(Some(LogTimestamp(starting + interval))).await.unwrap();
        }

        let (punish, downtime) = client.has_downtime_violation(&alice).await.unwrap().unwrap();
        assert_eq!(punish, false);
        assert_eq!(downtime, 0);

        let (punish, downtime) = client.has_downtime_violation(&bob).await.unwrap().unwrap();
        assert_eq!(punish, false); // Downtime, but does not exceed `max_downtime`.
        assert_eq!(downtime, 20);

        client.drop().await;
    }
}
