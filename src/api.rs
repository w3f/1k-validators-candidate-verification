use crate::database::{MongoClient, Timetable};
use crate::system::Network;
use actix_web::{get, web, App, HttpResponse, HttpServer, ResponseError, Result};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RestApiConfig {
    pub listen_addr: String,
    pub port: usize,
    pub endpoints: EndpointConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EndpointConfig {
    pub health: bool,
    pub downtime: bool,
}

#[derive(Clone)]
struct MongoState {
    client: MongoClient,
    scoped: HashMap<Network, MongoClient>,
}

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("Error: {}", error)]
pub struct WebError {
    error: String,
}

impl ResponseError for WebError {}

impl WebError {
    fn custom(msg: String) -> Self {
        WebError { error: msg }
    }
    fn internal() -> Self {
        Self::custom("Internal Error. Please contact admin.".to_string())
    }
}

#[derive(Deserialize)]
struct DowntimeQuery {
    network: Network,
    name: Option<String>,
}

#[get("/health")]
async fn health() -> HttpResponse {
    HttpResponse::Ok().body("OK")
}

#[get("/downtime")]
async fn downtime(
    query: web::Query<DowntimeQuery>,
    state: web::Data<MongoState>,
) -> Result<web::Json<Vec<Timetable>>> {
    let client = if let Some(scoped_client) = state.scoped.get(&query.network) {
        scoped_client
    } else {
        &state.client
    };

    let store = client.get_time_table_store_reader(&query.network);

    let who = if let Some(name) = &query.name {
        Some(name.as_str())
    } else {
        None
    };

    store
        .find_entries(who)
        .await
        .map(|t| web::Json(t))
        .map_err(|_| WebError::internal().into())
}

pub async fn start_rest_api(
    config: RestApiConfig,
    db_uri: &str,
    db_name: &str,
    db_scopes: Vec<(Network, String, String)>,
) -> std::result::Result<(), anyhow::Error> {
    let mut scoped = HashMap::new();
    for (network, db_uri, db_name) in db_scopes {
        scoped.insert(network, MongoClient::new(&db_uri, &db_name).await?);
    }

    let state = MongoState {
        client: MongoClient::new(db_uri, db_name).await?,
        scoped: scoped,
    };

    let listen_addr = config.listen_addr;
    let port = config.port;
    let endpoints = config.endpoints;

    HttpServer::new(move || {
        let mut app = App::new().data(state.clone());

        if endpoints.downtime {
            app = app.service(downtime);
        }
        if endpoints.health {
            app = app.service(health);
        }

        app
    })
    .bind(&format!("{}:{}", listen_addr, port))?
    .shutdown_timeout(5)
    .run()
    .await
    .map_err(|err| err.into())
}
