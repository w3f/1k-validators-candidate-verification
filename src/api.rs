use crate::database::{MongoClient, Timetable};
use crate::system::Network;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder, ResponseError, Result};

#[derive(Clone)]
struct MongoState {
    client: MongoClient,
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

#[get("/uptime/{network}/{who}")]
async fn handler(
    path: web::Path<(Network, String)>,
    state: web::Data<MongoState>,
) -> Result<web::Json<Vec<Timetable>>> {
    let (network, who) = path.into_inner();
    let store = state.client.get_time_table_store_reader(&network);

    let who = match who.as_str() {
        "all" => None,
        name @ _ => Some(name),
    };

    store
        .find_entries(who)
        .await
        .map(|t| web::Json(t))
        .map_err(|err| WebError::internal().into())
}

pub async fn start_rest_api(addr: &'static str) -> std::result::Result<(), anyhow::Error> {
    let state = MongoState {
        client: MongoClient::new("mongodb://localhost:27017/", "downtime_tracking").await?,
    };

    HttpServer::new(move || App::new().data(state.clone()).service(handler))
        .bind(addr)?
        .shutdown_timeout(5)
        .run()
        .await
        .map_err(|err| err.into())
}
