#[macro_use]
extern crate anyhow;

use futures::{StreamExt, SinkExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

mod events;

const DEFAULT_TELEMETRY: &'static str = "wss://telemetry-backend.w3f.community/feed";

type Result<T> = std::result::Result<T, anyhow::Error>;

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

impl Default for Chain {
    fn default() -> Self {
        Chain::Polkadot
    }
}

async fn run_listener(url: Option<&str>, chain: Option<Chain>) -> Result<()> {
    let url = url.unwrap_or(DEFAULT_TELEMETRY);
    let chain = chain.unwrap_or(Default::default());

    // Open stream.
    let (mut stream, _) = connect_async(url)
        .await
        .map_err(|err| anyhow!("Failed to connect to telemetry server: {:?}", err))?;

    // Subscribe to specified chain.
    stream
        .send(Message::text(format!("subscribe:{}", chain.as_ref())))
        .await
        .map_err(|err| anyhow!("Failed to subscribe to chain {}: {:?}", chain.as_ref(), err))?;

    while let Some(msg) = stream.next().await {
        match msg? {
            Message::Binary(content) => {
                println!(">> {}", String::from_utf8_lossy(&content));
            }
            _ => {},
        }
    }

    Ok(())
}
