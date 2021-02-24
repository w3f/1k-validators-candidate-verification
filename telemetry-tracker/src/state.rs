use crate::events::{MessageEvent, NodeId, NodeName};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegisteredStash;
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegisteredController;
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogTimestamp(u64);

impl LogTimestamp {
    pub fn new() -> Self {
        LogTimestamp(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs(),
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub name: Option<NodeName>,
    pub stash: Option<RegisteredStash>,
    pub controller: Option<RegisteredController>,
    pub last_event_log: LogTimestamp,
    pub event_logs: Vec<EventLog>,
}

impl NodeInfo {
    pub fn new(id: NodeId, name: Option<NodeName>) -> Self {
        NodeInfo {
            id: id,
            name: name,
            stash: None,
            controller: None,
            last_event_log: LogTimestamp::new(),
            event_logs: vec![],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventLog {
    pub timestamp: LogTimestamp,
    pub event: MessageEvent,
}
