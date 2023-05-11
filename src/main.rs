extern crate git2;

use actix_web::{get, HttpResponse, Responder};
use futures::stream::StreamExt;
use git2::build::RepoBuilder;
use git2::{FetchOptions, RemoteCallbacks};
use kube::{api::ListParams, client::Client, runtime::Controller, Api};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tracing::info;
use tracing_actix_web::TracingLogger;

mod controller;
mod finalizer;
mod project;
pub mod project_crd;
mod repository;

use crate::project_crd::Project;
use crate::repository::Repository;

#[derive(Serialize, Deserialize)]
struct Health {
    status: String,
}

#[get("/health")]
pub async fn health() -> impl Responder {
    HttpResponse::Ok().json(Health {
        status: "ok".to_string(),
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    //init tracing with env_logger
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .json()
        .init();

    //init dotenv
    dotenv::dotenv().ok();

    let kubernetes_client = Client::try_default()
        .await
        .expect("Failed to create client");

    let crd_api: Api<Project> = Api::all(kubernetes_client.clone());

    let context: Arc<controller::ContextData> = Arc::new(controller::ContextData {
        client: kubernetes_client.clone(),
    });

    //start server for health check
    let srv = actix_web::HttpServer::new(|| {
        actix_web::App::new()
            .wrap(TracingLogger::default())
            .service(health)
    })
    .bind("0.0.0.0:8080")
    .expect("Failed to bind to port 8080")
    .shutdown_timeout(5);

    let controller = Controller::new(crd_api.clone(), ListParams::default())
        .run(controller::reconcile, controller::on_error, context)
        .for_each(|reconciliation_result| async move {
            match reconciliation_result {
                Ok(echo_resource) => {
                    info!("Reconciliation successful. Resource: {:?}", echo_resource);
                }
                Err(reconciliation_err) => {
                    eprintln!("Reconciliation error: {:?}", reconciliation_err)
                }
            }
        });

    tokio::join!(controller, srv.run()).1?;
    Ok(())
}
