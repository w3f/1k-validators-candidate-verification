use super::Result;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeId(u64);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeName(String);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeImplementation(String);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeVersion(String);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Address(String);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Timestamp(f64);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockNumber(u64);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockHash(String);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockTime(Milliseconds);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkId(String);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PeerCount(usize);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionCount(usize);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Milliseconds(u64);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PropagationTime(Milliseconds);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BytesPerSecond(f64);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Latitude(f64);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Longitude(f64);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct City(String);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UploadSpeed(Vec<BytesPerSecond>);
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DownloadSpeed(Vec<BytesPerSecond>);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", content = "content")]
#[serde(rename_all = "snake_case")]
pub enum MessageEvent {
    AddedNode(AddedNodeEvent),
    Hardware(HardwareEvent),
    NodeStats(NodeStatsEvent),
}

impl MessageEvent {
    pub fn node_id(&self) -> &NodeId {
        match self {
            MessageEvent::AddedNode(event) => &event.node_id,
            MessageEvent::Hardware(event) => &event.node_id,
            MessageEvent::NodeStats(event) => &event.node_id,
        }
    }
}

impl MessageEvent {
    pub fn from_json(val: &Vec<u8>) -> Result<Vec<MessageEvent>> {
        let parsed: Vec<Value> = serde_json::from_slice(val)?;
        let mut index = 0;

        if parsed.len() == 0 || parsed.len() % 2 != 0 {
            return Err(anyhow!("invalid JSON data"));
        }

        let mut events = vec![];

        while index < parsed.len() - 1 {
            let action = serde_json::from_value(parsed[index].clone())?;
            index += 1;
            let payload = parsed[index].clone();

            if let Some(event) = match action {
                3 => Some(MessageEvent::AddedNode(
                    serde_json::from_value::<AddedNodeEventRaw>(payload)?.into(),
                )),
                8 => Some(MessageEvent::NodeStats(
                    serde_json::from_value::<NodeStatsEventRaw>(payload)?.into(),
                )),
                9 => Some(MessageEvent::Hardware(
                    serde_json::from_value::<HardwareEventRaw>(payload)?.into(),
                )),
                _ => None,
            } {
                events.push(event)
            }

            index += 1;
        }

        Ok(events)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HardwareEvent {
    node_id: NodeId,
    hardware: NodeHardware,
}

impl From<HardwareEventRaw> for HardwareEvent {
    fn from(val: HardwareEventRaw) -> Self {
        let val = val.0;

        HardwareEvent {
            node_id: val.0,
            hardware: NodeHardware {
                upload: val.1 .0,
                download: val.1 .1,
                chart_stamps: val.1 .2,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HardwareEventRaw(
    (
        NodeId,
        // NodeHardware
        (UploadSpeed, DownloadSpeed, Vec<Timestamp>),
    ),
);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeStatsEvent {
    node_id: NodeId,
    stats: NodeStats,
}

impl From<NodeStatsEventRaw> for NodeStatsEvent {
    fn from(val: NodeStatsEventRaw) -> Self {
        let val = val.0;

        NodeStatsEvent {
            node_id: val.0,
            stats: NodeStats {
                peers: val.1 .0,
                txcount: val.1 .1,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeStatsEventRaw(
    (
        NodeId,
        // NodeStats
        (PeerCount, TransactionCount),
    ),
);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AddedNodeEventRaw(
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
        (UploadSpeed, DownloadSpeed, Vec<Timestamp>),
        // BlockDetails
        (
            BlockNumber,
            BlockHash,
            BlockTime,
            Timestamp,
            Option<PropagationTime>,
        ),
        // NodeLocation
        Option<(Latitude, Longitude, City)>,
        // Timestamp
        Option<Timestamp>,
    ),
);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AddedNodeEvent {
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

impl From<AddedNodeEventRaw> for AddedNodeEvent {
    fn from(val: AddedNodeEventRaw) -> Self {
        let val = val.0;

        AddedNodeEvent {
            node_id: val.0,
            details: NodeDetails {
                name: val.1 .0,
                implementation: val.1 .1,
                version: val.1 .2,
                address: val.1 .3,
                network_id: val.1 .4,
            },
            stats: NodeStats {
                peers: val.2 .0,
                txcount: val.2 .1,
            },
            io: NodeIO {
                used_state_cache_size: val.3.get(0).unwrap_or(&vec![]).clone(),
            },
            hardware: NodeHardware {
                upload: val.4 .0,
                download: val.4 .1,
                chart_stamps: val.4 .2,
            },
            best: BlockDetails {
                block_number: val.5 .0,
                block_hash: val.5 .1,
                block_time: val.5 .2,
                block_timestamp: val.5 .3,
                propagation_time: val.5 .4,
            },
            location: val.6.map(|val| NodeLocation {
                latitude: val.0,
                longitude: val.1,
                city: val.2,
            }),
            startup_time: val.7,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeDetails {
    pub name: NodeName,
    pub implementation: NodeImplementation,
    pub version: NodeVersion,
    pub address: Option<Address>,
    pub network_id: Option<NetworkId>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeStats {
    pub peers: PeerCount,
    pub txcount: TransactionCount,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeIO {
    pub used_state_cache_size: Vec<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockDetails {
    pub block_number: BlockNumber,
    pub block_hash: BlockHash,
    pub block_time: BlockTime,
    pub block_timestamp: Timestamp,
    pub propagation_time: Option<PropagationTime>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Block {
    #[serde(rename = "best")]
    pub hash: BlockHash,
    pub height: BlockNumber,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeHardware {
    /// Upload uses means
    pub upload: UploadSpeed,
    /// Download uses means
    pub download: DownloadSpeed,
    /// Stampchange uses means
    pub chart_stamps: Vec<Timestamp>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeLocation {
    pub latitude: Latitude,
    pub longitude: Longitude,
    pub city: City,
}
