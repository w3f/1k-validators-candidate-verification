use crate::events::{MessageEvent, NodeId, NodeName};

#[derive(Deserialize, Debug, Clone)]
pub struct RegisteredStash;
#[derive(Deserialize, Debug, Clone)]
pub struct RegisteredController;
#[derive(Deserialize, Debug, Clone)]
pub struct LogTimestamp;

#[derive(Deserialize, Debug, Clone)]
pub struct NodeInfo {
    node_id: NodeId,
    name: NodeName,
    stash: Option<RegisteredStash>,
    controller: Option<RegisteredController>,
    last_event_log: LogTimestamp,
    event_logs: Vec<EventLog>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EventLog {
    timestamp: LogTimestamp,
    event: MessageEvent,
}
