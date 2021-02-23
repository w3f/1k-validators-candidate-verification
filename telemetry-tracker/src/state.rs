use crate::events::{NodeId, NodeName, MessageEvent};

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
    last_event: LogTimestamp,
    edit_logs: Vec<EditorLog>,
    event_logs: Vec<EventLog>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EditorLog {
    editor_task: EditorTask,
    timestamp: LogTimestamp,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum EditorTask {
    TelemetryWatcher,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EventLog {
    timestamp: LogTimestamp,
    event: MessageEvent,
}
