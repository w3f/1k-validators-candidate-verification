use super::Result;
use serde_json::Value;

#[derive(Deserialize, Debug, Clone)]
pub struct NodeId(u64);
#[derive(Deserialize, Debug, Clone)]
pub struct NodeName(String);
#[derive(Deserialize, Debug, Clone)]
pub struct NodeImplementation(String);
#[derive(Deserialize, Debug, Clone)]
pub struct NodeVersion(String);
#[derive(Deserialize, Debug, Clone)]
pub struct Address(String);
#[derive(Deserialize, Debug, Clone)]
pub struct Timestamp(f64);
#[derive(Deserialize, Debug, Clone)]
pub struct BlockNumber(u64);
#[derive(Deserialize, Debug, Clone)]
pub struct BlockHash(String);
#[derive(Deserialize, Debug, Clone)]
pub struct NetworkId(String);
#[derive(Deserialize, Debug, Clone)]
pub struct PeerCount(usize);
#[derive(Deserialize, Debug, Clone)]
pub struct TransactionCount(usize);
#[derive(Deserialize, Debug, Clone)]
pub struct Milliseconds(u64);
#[derive(Deserialize, Debug, Clone)]
pub struct PropagationTime(Milliseconds);
#[derive(Deserialize, Debug, Clone)]
pub struct BytesPerSecond(f64);
#[derive(Deserialize, Debug, Clone)]
pub struct Latitude(f64);
#[derive(Deserialize, Debug, Clone)]
pub struct Longitude(f64);
#[derive(Deserialize, Debug, Clone)]
pub struct City(String);

#[derive(Debug, Clone)]
pub enum MessageEvent {
    AddedNode(AddedNode),
}

impl MessageEvent {
    pub fn from_json(val: &Vec<u8>) -> Result<Vec<MessageEvent>> {
        let parsed: Vec<Value> = serde_json::from_slice(val)?;
        let mut index = 0;

        if parsed.len() == 0 || parsed.len() % 2 != 0 {
            return Err(anyhow!("invalid JSON data"));
        }

        let mut messages = vec![];

        while index < parsed.len() - 1 {
            let action = serde_json::from_value(parsed[index].clone())?;
            index += 1;
            let payload = parsed[index].clone();

            println!("ACTION: {}", action);
            match action {
                //3 => messages.push(MessageEvent::AddedNode(serde_json::from_value(payload)?)),
                3 => {
                    serde_json::from_value::<AddedNodeRaw>(payload)?;
                }
                _ => {}
            }

            index += 1;
        }

        Ok(messages)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct AddedNodeRaw(
    (
        NodeId,
        // NodeDetails
        (
            NodeName,
            NodeImplementation,
            NodeVersion,
            Option<Address>,
            Option<NetworkId>,
        ),
        // NodeStats
        (PeerCount, TransactionCount),
        // NodeIO
        Vec<Vec<f64>>,
        // NodeHardware
        (Vec<BytesPerSecond>, Vec<BytesPerSecond>, Vec<Timestamp>),
        // BlockDetails
        (
            BlockNumber,
            BlockHash,
            Milliseconds,
            Timestamp,
            Option<PropagationTime>,
        ),
        // NodeLocation
        Option<(Latitude, Longitude, City)>,
        // Timestamp
        Option<Timestamp>,
    ),
);

#[derive(Deserialize, Debug, Clone)]
pub struct AddedNode {
    /// Node identifier
    node_id: NodeId,
    /// Static details
    details: NodeDetails,
    /// Basic stats
    stats: NodeStats,
    /// Node IO stats
    io: NodeIO,
    /// Hardware stats over time
    hardware: NodeHardware,
    /// Best block
    best: BlockDetails,
    /// Physical location details
    location: Option<NodeLocation>,
    /// Unix timestamp for when node started up (falls back to connection time)
    startup_time: Option<Timestamp>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NodeDetails {
    pub chain: Box<str>,
    pub name: Box<str>,
    pub implementation: Box<str>,
    pub version: Box<str>,
    pub validator: Option<Box<str>>,
    pub network_id: Option<Box<str>>,
    pub startup_time: Option<Box<str>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NodeStats {
    pub peers: u64,
    pub txcount: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NodeIO {
    pub used_state_cache_size: MeanList<f32>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MeanList<T> {
    period_sum: T,
    period_count: u8,
    mean_index: u8,
    means: [T; 20],
    ticks_per_mean: u8,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BlockDetails {
    pub block: Block,
    pub block_time: u64,
    pub block_timestamp: u64,
    pub propagation_time: Option<u64>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Block {
    #[serde(rename = "best")]
    pub hash: BlockHash,
    pub height: BlockNumber,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NodeHardware {
    /// Upload uses means
    pub upload: MeanList<f64>,
    /// Download uses means
    pub download: MeanList<f64>,
    /// Stampchange uses means
    pub chart_stamps: MeanList<f64>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NodeLocation {
    pub latitude: f32,
    pub longitude: f32,
    pub city: Box<str>,
}
