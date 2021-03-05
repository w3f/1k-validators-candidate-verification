use crate::database::{CandidateState, MongoClient, TimetableStoreConfig};
use crate::events::{NodeName, TelemetryEvent};
use crate::judge::RequirementsProceeding;
use crate::{jury::RequirementsConfig, Result};
use futures::{SinkExt, StreamExt};
use std::fs::read_to_string;
use substrate_subxt::sp_core::crypto::Ss58Codec;
use substrate_subxt::{DefaultNodeRuntime, KusamaRuntime};
use tokio::time::{self, Duration};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

const DOWNTIME_PROCESSOR_TIMEOUT: u64 = 60;
const UNEXPECTED_EXIT_TIMEOUT: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Network {
    Polkadot,
    Kusama,
}

impl Network {
    // A string with the first letter capitalized. Required for telemetry subscription.
    pub fn as_subscription(&self) -> &str {
        match self {
            Network::Polkadot => "Polkadot",
            Network::Kusama => "Kusama",
        }
    }
}

impl AsRef<str> for Network {
    fn as_ref(&self) -> &str {
        match self {
            Network::Polkadot => "polkadot",
            Network::Kusama => "kusama",
        }
    }
}

pub fn read_candidates(path: &str, network: Network) -> Result<Vec<Candidate>> {
    #[derive(Debug, Clone, Deserialize, Serialize)]
    struct RawCandidate {
        name: NodeName,
        stash: String,
    }

    let candidates = serde_yaml::from_str::<Vec<RawCandidate>>(&read_to_string(path)?)?
        .into_iter()
        .map(|raw| Candidate::new(raw.stash, raw.name, network))
        .collect();

    Ok(candidates)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelemetryWatcherConfig {
    pub db_uri: String,
    pub db_name: String,
    pub telemetry_host: String,
    pub network: Network,
    pub store_behavior: StoreBehavior,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", content = "config", rename_all = "snake_case")]
pub enum StoreBehavior {
    Store,
    Counter(TimetableStoreConfig),
}

pub async fn run_telemetry_watcher(config: TelemetryWatcherConfig) -> Result<()> {
    async fn local(config: &TelemetryWatcherConfig) -> Result<()> {
        info!("Opening MongoDB client to database {}", &config.db_name);
        let client = MongoClient::new(&config.db_uri, &config.db_name).await?;

        let store = client.get_telemetry_event_store(&config.network);
        let mut tracker =
            client.get_time_table_store(TimetableStoreConfig::dummy(), &config.network);

        info!(
            "Connecting to telemetry server {} ({})",
            config.telemetry_host,
            config.network.as_ref()
        );
        let (mut stream, _) = connect_async(&config.telemetry_host)
            .await
            .map_err(|err| anyhow!("Failed to connect to telemetry server: {:?}", err))?;

        // Subscribe to specified network.
        info!("Subscribing to node events ({})", config.network.as_ref());
        stream
            .send(Message::text(format!(
                "subscribe:{}",
                config.network.as_subscription()
            )))
            .await
            .map_err(|err| {
                anyhow!(
                    "Failed to subscribe to network {}: {:?}",
                    config.network.as_subscription(),
                    err
                )
            })?;

        match &config.store_behavior {
            StoreBehavior::Counter(track_config) => {
                info!(
                    "Starting downtime processing service ({})",
                    config.network.as_ref()
                );

                tracker = client.get_time_table_store(track_config.clone(), &config.network);
                let processor = tracker.clone();
                let network = config.network;

                tokio::spawn(async move {
                    loop {
                        if let Err(err) = processor.process_time_tables().await {
                            error!(
                                "Exiting downtime processing service: {:?} ({})",
                                err,
                                network.as_ref()
                            );
                            break;
                        }

                        time::sleep(Duration::from_secs(DOWNTIME_PROCESSOR_TIMEOUT)).await;
                    }
                });

                // TODO: If the downtime processor exists, the full application should exit.
            }
            _ => {}
        }

        info!("Starting event loop ({})", config.network.as_ref());

        while let Some(msg) = stream.next().await {
            match msg? {
                Message::Binary(content) => {
                    if let Ok(events) = TelemetryEvent::from_json(&content) {
                        for event in events {
                            debug!(
                                "NodeId {} (name \"{}\"): new '{}' event ({})",
                                event.node_id().as_num(),
                                event.node_name().map(|n| n.as_str()).unwrap_or("N/A"),
                                event.event_name(),
                                config.network.as_ref(),
                            );

                            match config.store_behavior {
                                StoreBehavior::Store => store.store_event(event).await?,
                                StoreBehavior::Counter(_) => tracker.track_event(event).await?,
                            }
                        }
                    } else {
                        error!("Failed to deserialize telemetry event");
                    }
                }
                _ => {}
            }
        }

        Result::Ok(())
    }

    tokio::spawn(async move {
        loop {
            match local(&config).await {
                Ok(_) => info!("Telemetry connection dropped, restarting..."),
                Err(err) => {
                    error!(
                        "Telemetry watcher exited unexpectedly, restarting in {} seconds; {:?}",
                        UNEXPECTED_EXIT_TIMEOUT, err
                    );
                    time::sleep(Duration::from_secs(UNEXPECTED_EXIT_TIMEOUT)).await;
                    break;
                }
            }
        }
    });

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Candidate {
    stash: String,
    node_name: NodeName,
    network: Network,
}

impl Candidate {
    pub fn new(stash: String, node_name: NodeName, network: Network) -> Self {
        Candidate {
            stash: stash,
            node_name: node_name,
            network: network,
        }
    }
    pub fn stash_str(&self) -> &str {
        self.stash.as_str()
    }
    pub fn node_name(&self) -> &NodeName {
        &self.node_name
    }
    pub fn to_account_id<T: Ss58Codec>(&self) -> Result<T> {
        Ok(T::from_ss58check(&self.stash).map_err(|_err| {
            anyhow!("Failed to convert presumed SS58 string into a NetworkAccount")
        })?)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequirementsProceedingConfig {
    pub db_uri: String,
    pub db_name: String,
    pub rpc_hostname: String,
    pub requirements_config: RequirementsConfig<u128>,
    pub network: Network,
}

pub async fn run_requirements_proceeding(
    config: RequirementsProceedingConfig,
    candidates: Vec<Candidate>,
) -> Result<()> {
    info!("Opening MongoDB client ({})", config.network.as_ref());
    let store = MongoClient::new(&config.db_uri, &config.db_name).await?;

    let candidate_store = store.get_candidate_state_store(&config.network);
    let telemetry_store = store.get_telemetry_event_store(&config.network);

    match config.network {
        Network::Polkadot => {
            let proceeding = RequirementsProceeding::<DefaultNodeRuntime>::new(
                &config.rpc_hostname,
                config.requirements_config,
                telemetry_store,
            )
            .await?;

            for candidate in candidates {
                let state = candidate_store
                    .fetch_candidate_state(&candidate)
                    .await?
                    .unwrap_or(CandidateState::new(candidate.clone()));

                let report = proceeding.proceed_requirements(state).await?;

                candidate_store
                    .store_requirements_report(&candidate, report)
                    .await?;
            }
        }
        Network::Kusama => {
            let proceeding = RequirementsProceeding::<KusamaRuntime>::new(
                &config.rpc_hostname,
                config.requirements_config,
                telemetry_store,
            )
            .await?;

            for candidate in candidates {
                let state = candidate_store
                    .fetch_candidate_state(&candidate)
                    .await?
                    .unwrap_or(CandidateState::new(candidate.clone()));

                let report = proceeding.proceed_requirements(state).await?;

                candidate_store
                    .store_requirements_report(&candidate, report)
                    .await?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Candidate {
        pub fn alice() -> Self {
            Candidate::new("".to_string(), NodeName::alice(), Network::Polkadot)
        }
        pub fn bob() -> Self {
            Candidate::new("".to_string(), NodeName::bob(), Network::Polkadot)
        }
        pub fn eve() -> Self {
            Candidate::new("".to_string(), NodeName::eve(), Network::Polkadot)
        }
    }

    #[tokio::test]
    #[ignore]
    async fn telemetry() {
        let (mut stream, _) = connect_async("wss://telemetry-backend.w3f.community/feed")
            .await
            .unwrap();

        // Subscribe to specified network.
        stream
            .send(Message::text(format!(
                "subscribe:{}",
                Network::Polkadot.as_ref()
            )))
            .await
            .unwrap();

        while let Some(msg) = stream.next().await {
            match msg.unwrap() {
                Message::Binary(content) => {
                    if let Ok(events) = TelemetryEvent::from_json(&content) {
                        for event in events {
                            println!("\n\n{}", serde_json::to_string(&event).unwrap());
                        }
                    } else {
                        error!("Failed to deserialize telemetry event");
                    }
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn requirements_proceeding() {
        //env_logger::init();

        let config = RequirementsProceedingConfig {
            db_uri: "mongodb://localhost:27017/".to_string(),
            db_name: "test_candidate_requirements".to_string(),
            network: Network::Kusama,
            rpc_hostname: "wss://kusama-rpc.polkadot.io".to_string(),
            requirements_config: RequirementsConfig {
                max_commission: 10,
                min_bonded_amount: 10000,
                node_activity_timespan: 0,
                max_node_activity_diff: 0,
            },
        };

        let candidates = vec![Candidate::new(
            "FyRaMYvPqpNGq6PFGCcUWcJJWKgEz29ZFbdsnoNAczC2wJZ".to_string(),
            NodeName::new("Alice".to_string()),
            Network::Kusama,
        )];

        run_requirements_proceeding(config, candidates)
            .await
            .unwrap();
    }
}
