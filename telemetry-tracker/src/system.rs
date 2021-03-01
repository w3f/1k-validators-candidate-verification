use crate::{Result, jury::RequirementsConfig};
use crate::database::MongoClient;
use crate::events::TelemetryEvent;
use crate::judge::RequirementsProceeding;
use substrate_subxt::Runtime;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use std::convert::TryInto;

pub enum Chain {
    Polkadot,
    Kusama,
}

impl AsRef<str> for Chain {
    fn as_ref(&self) -> &str {
        match self {
            Chain::Polkadot => "Polkadot",
            Chain::Kusama => "Kusama",
        }
    }
}

pub struct TelemetryWatcherConfig {
    enabled: bool,
    uri: String,
    database: String,
    telemetry_host: String,
    chain: Chain,
}

async fn run_telemetry_watcher(config: TelemetryWatcherConfig) -> Result<()> {
    info!("Opening MongoDB client");
    let client = MongoClient::new(&config.uri, &config.database)
        .await?
        .get_telemetry_event_store();

    info!("Connecting to telemetry server");
    let (mut stream, _) = connect_async(&config.telemetry_host)
        .await
        .map_err(|err| anyhow!("Failed to connect to telemetry server: {:?}", err))?;

    // Subscribe to specified chain.
    info!("Subscribing to {} chain", config.chain.as_ref());
    stream
        .send(Message::text(format!(
            "subscribe:{}",
            config.chain.as_ref()
        )))
        .await
        .map_err(|err| {
            anyhow!(
                "Failed to subscribe to chain {}: {:?}",
                config.chain.as_ref(),
                err
            )
        })?;

    info!("Starting event loop");
    tokio::spawn(async move {
        let local = || async move {
            let store = client;

            while let Some(msg) = stream.next().await {
                match msg? {
                    Message::Binary(content) => {
                        if let Ok(events) = TelemetryEvent::from_json(&content) {
                            for event in events {
                                store.store_event(event).await?;
                            }
                        } else {
                            error!("Failed to deserialize telemetry event");
                        }
                    }
                    _ => {}
                }
            }

            Result::Ok(())
        };

        error!("Exiting telemetry watcher task: {:?}", local().await);
    });

    Ok(())
}

pub struct Candidate {
    stash: String,
}

impl Candidate {
    pub fn stash_str(&self) -> &str {
        self.stash.as_str()
    }
}

pub struct RequirementsProceedingConfig {
    enabled: bool,
    db_uri: String,
    db_name: String,
    chain: Chain,
    rpc_hostname: String,
    requirements_config: RequirementsConfig<u128>,
    candidates: Vec<Candidate>,
}

use substrate_subxt::{KusamaRuntime, DefaultNodeRuntime};

async fn run_requirements_proceeding<T>(config: RequirementsProceedingConfig) -> Result<()> {
    info!("Opening MongoDB client");
    let client = MongoClient::new(&config.db_uri, &config.db_name)
        .await?
        .get_telemetry_event_store();


    match config.chain {
        Chain::Polkadot => {
            let proceeding = RequirementsProceeding::<DefaultNodeRuntime>::new(&config.rpc_hostname).await?;

            for candidate in config.candidates {
                proceeding.proceed_requirements(candidate.try_into()?, config.requirements_config).await?;
            }
        }
        Chain::Kusama => {
            let proceeding = RequirementsProceeding::<KusamaRuntime>::new(&config.rpc_hostname).await;
        }
    }

    Ok(())
}

#[tokio::test]
async fn telemetry() {
    let (mut stream, _) = connect_async("wss://telemetry-backend.w3f.community/feed")
        .await
        .unwrap();

    // Subscribe to specified chain.
    stream
        .send(Message::text(format!(
            "subscribe:{}",
            Chain::Polkadot.as_ref()
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
