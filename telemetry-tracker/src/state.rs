use crate::events::{MessageEvent, NodeId, NodeName};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegisteredStash;
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegisteredController;
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogTimestamp(u64);

impl LogTimestamp {
    fn new() -> Self {
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
    node_id: NodeId,
    node_name: NodeName,
    stash: Option<RegisteredStash>,
    controller: Option<RegisteredController>,
    last_event_log: LogTimestamp,
    event_logs: Vec<EventLog>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventLog {
    timestamp: LogTimestamp,
    event: MessageEvent,
}
