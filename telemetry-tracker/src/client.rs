use crate::Result;
use crate::events::MessageEvent;
use mongodb::Client;
use tokio_tungstenite::tungstenite::Message;

pub struct MongoClient {
    client: Client,
}

impl MongoClient {
    async fn new(uri: &str) -> Result<Self> {
        Ok(MongoClient {
            client: Client::with_uri_str(uri).await?,
        })
    }
}

#[async_trait]
pub trait StoreTelemetryEvents {
    async fn store_event(event: MessageEvent) -> Result<()>;
}

#[async_trait]
impl StoreTelemetryEvents for MongoClient {
    async fn store_event(event: MessageEvent) -> Result<()> {
        unreachable!()
    }
}