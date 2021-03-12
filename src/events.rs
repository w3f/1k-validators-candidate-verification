use super::Result;
use serde_json::Value;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct NodeId(i64);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct NodeName(String);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct NodeImplementation(String);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct NodeVersion(String);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct Address(String);
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct Timestamp(f64);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct BlockNumber(i64);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct BlockHash(String);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct BlockTime(Milliseconds);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct NetworkId(String);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct PeerCount(i64);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct TransactionCount(i64);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct Milliseconds(i64);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct PropagationTime(Milliseconds);
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct BytesPerSecond(f64);
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct Latitude(f64);
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct Longitude(f64);
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default, Deserialize, Serialize)]
pub struct City(String);
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct UploadSpeed(Vec<BytesPerSecond>);
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct DownloadSpeed(Vec<BytesPerSecond>);

impl NodeName {
    pub fn new(name: String) -> Self {
        NodeName(name)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl NodeId {
    pub fn as_num(&self) -> i64 {
        self.0
    }
}

impl NodeVersion {
    // Converts `0.8.29-2494dec2-x86_64-linux-gnu` into `0.8.29`.
    pub fn strip_os_suffix(self) -> Result<NodeVersion> {
        let version = self.0.split("-").nth(0).ok_or(anyhow!(
            "failed to strip client version OS suffix, invalid format"
        ))?;

        Ok(NodeVersion(version.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", content = "content")]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEvent {
    AddedNode(AddedNodeEvent),
    Hardware(HardwareEvent),
    NodeStats(NodeStatsEvent),
    #[cfg(test)]
    TestMessage(NodeId, String),
}

impl TelemetryEvent {
    pub fn node_id(&self) -> &NodeId {
        match self {
            TelemetryEvent::AddedNode(event) => &event.node_id,
            TelemetryEvent::Hardware(event) => &event.node_id,
            TelemetryEvent::NodeStats(event) => &event.node_id,
            #[cfg(test)]
            TelemetryEvent::TestMessage(node_id, _) => node_id,
        }
    }
    pub fn node_name(&self) -> Option<&NodeName> {
        match self {
            TelemetryEvent::AddedNode(event) => Some(&event.details.name),
            _ => None,
        }
    }
    pub fn event_name(&self) -> &str {
        match self {
            TelemetryEvent::AddedNode(_) => "AddedNode",
            TelemetryEvent::Hardware(_) => "Hardware",
            TelemetryEvent::NodeStats(_) => "NodeStats",
            #[cfg(test)]
            TelemetryEvent::TestMessage(_, _) => "TestMessage",
        }
    }
}

impl TelemetryEvent {
    pub fn from_json(val: &Vec<u8>) -> Result<Vec<TelemetryEvent>> {
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
                3 => Some(TelemetryEvent::AddedNode(
                    serde_json::from_value::<AddedNodeEventRaw>(payload)?.into(),
                )),
                8 => Some(TelemetryEvent::NodeStats(
                    serde_json::from_value::<NodeStatsEventRaw>(payload)?.into(),
                )),
                9 => Some(TelemetryEvent::Hardware(
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

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct HardwareEventRaw(
    (
        NodeId,
        // NodeHardware
        (UploadSpeed, DownloadSpeed, Vec<Timestamp>),
    ),
);

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct NodeStatsEventRaw(
    (
        NodeId,
        // NodeStats
        (PeerCount, TransactionCount),
    ),
);

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AddedNodeEvent {
    /// Node identifier
    pub node_id: NodeId,
    /// Static details
    pub details: NodeDetails,
    /// Basic stats
    pub stats: NodeStats,
    /// Node IO stats
    pub io: NodeIO,
    /// Hardware stats over time
    pub hardware: NodeHardware,
    /// Best block
    pub best: BlockDetails,
    /// Physical location details
    pub location: Option<NodeLocation>,
    /// Unix timestamp for when node started up (falls back to connection time)
    pub startup_time: Option<Timestamp>,
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

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct NodeDetails {
    pub name: NodeName,
    pub implementation: NodeImplementation,
    pub version: NodeVersion,
    pub address: Option<Address>,
    pub network_id: Option<NetworkId>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct NodeStats {
    pub peers: PeerCount,
    pub txcount: TransactionCount,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct NodeIO {
    pub used_state_cache_size: Vec<f64>,
}

impl Default for NodeIO {
    fn default() -> Self {
        NodeIO {
            used_state_cache_size: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct BlockDetails {
    pub block_number: BlockNumber,
    pub block_hash: BlockHash,
    pub block_time: BlockTime,
    pub block_timestamp: Timestamp,
    pub propagation_time: Option<PropagationTime>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Block {
    #[serde(rename = "best")]
    pub hash: BlockHash,
    pub height: BlockNumber,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct NodeHardware {
    /// Upload uses means
    pub upload: UploadSpeed,
    /// Download uses means
    pub download: DownloadSpeed,
    /// Stampchange uses means
    pub chart_stamps: Vec<Timestamp>,
}

impl Default for NodeHardware {
    fn default() -> Self {
        NodeHardware {
            upload: UploadSpeed(vec![]),
            download: DownloadSpeed(vec![]),
            chart_stamps: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct NodeLocation {
    pub latitude: Latitude,
    pub longitude: Longitude,
    pub city: City,
}

impl Default for NodeLocation {
    fn default() -> Self {
        NodeLocation {
            latitude: Latitude(0.0),
            longitude: Longitude(0.0),
            city: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    impl NodeId {
        pub fn alice() -> Self {
            NodeId(1)
        }
        pub fn bob() -> Self {
            NodeId(2)
        }
        pub fn eve() -> Self {
            NodeId(3)
        }
    }

    impl From<i64> for NodeId {
        fn from(val: i64) -> Self {
            NodeId(val)
        }
    }

    impl From<String> for NodeVersion {
        fn from(val: String) -> Self {
            NodeVersion(val)
        }
    }

    #[allow(dead_code)]
    impl HardwareEvent {
        pub fn alice() -> Self {
            HardwareEvent {
                node_id: NodeId::alice(),
                hardware: Default::default(),
            }
        }
        pub fn bob() -> Self {
            HardwareEvent {
                node_id: NodeId::bob(),
                hardware: Default::default(),
            }
        }
        pub fn eve() -> Self {
            HardwareEvent {
                node_id: NodeId::eve(),
                hardware: Default::default(),
            }
        }
    }

    #[allow(dead_code)]
    impl NodeStatsEvent {
        pub fn alice() -> Self {
            NodeStatsEvent {
                node_id: NodeId::alice(),
                stats: Default::default(),
            }
        }
        pub fn bob() -> Self {
            NodeStatsEvent {
                node_id: NodeId::bob(),
                stats: Default::default(),
            }
        }
        pub fn eve() -> Self {
            NodeStatsEvent {
                node_id: NodeId::eve(),
                stats: Default::default(),
            }
        }
    }

    impl NodeName {
        pub fn alice() -> Self {
            NodeName("alice".to_string())
        }
        pub fn bob() -> Self {
            NodeName("bob".to_string())
        }
        pub fn eve() -> Self {
            NodeName("eve".to_string())
        }
    }

    #[allow(dead_code)]
    impl AddedNodeEvent {
        pub fn alice() -> Self {
            AddedNodeEvent {
                node_id: NodeId::alice(),
                details: {
                    let mut details = NodeDetails::default();
                    details.name = NodeName::alice();
                    details
                },
                stats: Default::default(),
                io: Default::default(),
                hardware: Default::default(),
                best: Default::default(),
                location: Default::default(),
                startup_time: Default::default(),
            }
        }
        pub fn bob() -> Self {
            AddedNodeEvent {
                node_id: NodeId::bob(),
                details: {
                    let mut details = NodeDetails::default();
                    details.name = NodeName::bob();
                    details
                },
                stats: Default::default(),
                io: Default::default(),
                hardware: Default::default(),
                best: Default::default(),
                location: Default::default(),
                startup_time: Default::default(),
            }
        }
        pub fn eve() -> Self {
            AddedNodeEvent {
                node_id: NodeId::eve(),
                details: {
                    let mut details = NodeDetails::default();
                    details.name = NodeName::eve();
                    details
                },
                stats: Default::default(),
                io: Default::default(),
                hardware: Default::default(),
                best: Default::default(),
                location: Default::default(),
                startup_time: Default::default(),
            }
        }
    }
}
