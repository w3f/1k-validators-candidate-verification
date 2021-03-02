use crate::events::TelemetryEvent;
use crate::judge::RequirementsProceeding;
use crate::{
    database::{CandidateState, MongoClient},
    judge::NetworkAccount,
};
use crate::{jury::RequirementsConfig, Result};
use futures::{SinkExt, StreamExt};
use std::convert::TryInto;
use substrate_subxt::sp_core::crypto::Ss58Codec;
use substrate_subxt::{sp_runtime::AccountId32, Runtime};
use substrate_subxt::{DefaultNodeRuntime, KusamaRuntime};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Network {
    Polkadot,
    Kusama,
}

impl AsRef<str> for Network {
    fn as_ref(&self) -> &str {
        match self {
            Network::Polkadot => "Polkadot",
            Network::Kusama => "Kusama",
        }
    }
}

pub struct TelemetryWatcherConfig {
    enabled: bool,
    uri: String,
    database: String,
    telemetry_host: String,
    network: Network,
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

    // Subscribe to specified network.
    info!("Subscribing to {} network", config.network.as_ref());
    stream
        .send(Message::text(format!(
            "subscribe:{}",
            config.network.as_ref()
        )))
        .await
        .map_err(|err| {
            anyhow!(
                "Failed to subscribe to network {}: {:?}",
                config.network.as_ref(),
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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Candidate {
    stash: String,
    network: Network,
}

impl Candidate {
    pub fn stash_str(&self) -> &str {
        self.stash.as_str()
    }
    pub fn to_account_id<T: Ss58Codec>(&self) -> Result<T> {
        Ok(T::from_ss58check(&self.stash).map_err(|err| {
            anyhow!("Failed to convert presumed SS58 string into a NetworkAccount")
        })?)
    }
}

impl From<(String, Network)> for Candidate {
    fn from(val: (String, Network)) -> Self {
        Candidate {
            stash: val.0,
            network: val.1,
        }
    }
}

pub struct RequirementsProceedingConfig {
    enabled: bool,
    db_uri: String,
    db_name: String,
    network: Network,
    rpc_hostname: String,
    requirements_config: RequirementsConfig<u128>,
    candidates: Vec<Candidate>,
}

async fn run_requirements_proceeding(config: RequirementsProceedingConfig) -> Result<()> {
    info!("Opening MongoDB client");
    let store = MongoClient::new(&config.db_uri, &config.db_name)
        .await?
        .get_candidate_state_store();

    match config.network {
        Network::Polkadot => {
            let proceeding = RequirementsProceeding::<DefaultNodeRuntime>::new(
                &config.rpc_hostname,
                config.requirements_config,
            )
            .await?;

            for candidate in config.candidates {
                let state = store
                    .fetch_candidate_state(&candidate)
                    .await?
                    .unwrap_or(CandidateState::new(candidate.clone()));

                let report = proceeding.proceed_requirements(state).await?;

                store.store_requirements_report(&candidate, report).await?;
            }
        }
        Network::Kusama => {
            let proceeding = RequirementsProceeding::<KusamaRuntime>::new(
                &config.rpc_hostname,
                config.requirements_config,
            )
            .await?;

            for candidate in config.candidates {
                let state = store
                    .fetch_candidate_state(&candidate)
                    .await?
                    .unwrap_or(CandidateState::new(candidate.clone()));

                let report = proceeding.proceed_requirements(state).await?;

                store.store_requirements_report(&candidate, report).await?;
            }
        }
    }

    Ok(())
}

#[tokio::test]
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
async fn requirements_proceeding() {
    //env_logger::init();

    let config = RequirementsProceedingConfig {
        enabled: true,
        db_uri: "mongodb://localhost:27017/".to_string(),
        db_name: "test_candidate_requirements".to_string(),
        network: Network::Kusama,
        rpc_hostname: "wss://kusama-rpc.polkadot.io".to_string(),
        requirements_config: RequirementsConfig {
            commission: 10,
            bonded_amount: 10000,
        },
        candidates: vec![Candidate::from((
            "FyRaMYvPqpNGq6PFGCcUWcJJWKgEz29ZFbdsnoNAczC2wJZ".to_string(),
            Network::Kusama,
        ))],
    };

    run_requirements_proceeding(config).await.unwrap();
}
