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
    pub node_id: NodeId,
    pub stash: Option<RegisteredStash>,
    pub controller: Option<RegisteredController>,
    pub last_event_log: LogTimestamp,
    pub event_logs: Vec<EventLog>,
}

impl NodeInfo {
    pub fn from_node_id(id: NodeId) -> Self {
        NodeInfo {
            node_id: id,
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
