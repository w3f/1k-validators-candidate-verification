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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Candidate {
    stash: String,
    chain: Chain,
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

impl From<(String, Chain)> for Candidate {
    fn from(val: (String, Chain)) -> Self {
        Candidate {
            stash: val.0,
            chain: val.1,
        }
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

async fn run_requirements_proceeding(config: RequirementsProceedingConfig) -> Result<()> {
    info!("Opening MongoDB client");
    let store = MongoClient::new(&config.db_uri, &config.db_name)
        .await?
        .get_candidate_state_store();

    match config.chain {
        Chain::Polkadot => {
            let proceeding = RequirementsProceeding::<DefaultNodeRuntime>::new(
                &config.rpc_hostname,
                config.requirements_config,
            )
            .await?;

            for candidate in config.candidates {
                let state = store
                    .fetch_candidate_state(&candidate)
                    .await?
                    .unwrap_or(CandidateState::new(candidate));

                let report = proceeding.proceed_requirements(state).await?;

                store.store_requirements_report(report).await?;
            }
        }
        Chain::Kusama => {
            let proceeding = RequirementsProceeding::<KusamaRuntime>::new(
                &config.rpc_hostname,
                config.requirements_config,
            )
            .await?;

            for candidate in config.candidates {
                let state = store
                    .fetch_candidate_state(&candidate)
                    .await?
                    .unwrap_or(CandidateState::new(candidate));

                let report = proceeding.proceed_requirements(state).await?;

                store.store_requirements_report(report).await?;
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

#[tokio::test]
async fn requirements_proceeding() {
    //env_logger::init();

    let config = RequirementsProceedingConfig {
        enabled: true,
        db_uri: "mongodb://localhost:27017/".to_string(),
        db_name: "test_candidate_requirements".to_string(),
        chain: Chain::Kusama,
        rpc_hostname: "wss://kusama-rpc.polkadot.io".to_string(),
        requirements_config: RequirementsConfig {
            commission: 10,
            bonded_amount: 10000,
        },
        candidates: vec![Candidate::from((
            "FyRaMYvPqpNGq6PFGCcUWcJJWKgEz29ZFbdsnoNAczC2wJZ".to_string(),
            Chain::Kusama,
        ))],
    };

    run_requirements_proceeding(config).await.unwrap();
}
