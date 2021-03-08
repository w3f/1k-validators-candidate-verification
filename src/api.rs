use crate::Result;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};

#[get("/")]
async fn handler() -> impl Responder {
    HttpResponse::Ok().body("")
}

pub async fn start_rest_api(addr: &str) -> Result<()> {
    HttpServer::new(|| App::new().service(handler))
        .bind(addr)?
        .run()
        .await
        .map_err(|err| err.into())
}
