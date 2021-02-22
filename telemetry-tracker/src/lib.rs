#[macro_use]
extern crate anyhow;

use futures::SinkExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

mod events;

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

async fn run_listener(url: &str, chain: Chain) -> Result<()> {
    // Open stream.
    let (mut stream, _) = connect_async(url)
        .await
        .map_err(|err| anyhow!("Failed to connect to telemetry server: {:?}", err))?;

    // Subscribe to specified chain.
    stream
        .send(Message::text(format!("subscribe:{}", chain.as_ref())))
        .await
        .map_err(|err| anyhow!("Failed to subscribe to chain {}: {:?}", chain.as_ref(), err))?;

    Ok(())
}
