use crate::events::{NodeId, NodeName, TelemetryEvent};
use std::time::{SystemTime, UNIX_EPOCH};

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
