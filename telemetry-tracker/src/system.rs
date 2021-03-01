use crate::database::MongoClient;
use crate::events::TelemetryEvent;
use crate::Result;
use futures::{SinkExt, StreamExt};

use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

enum Chain {
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
