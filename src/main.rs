use actix_web::{get, HttpResponse, Responder};
use futures::stream::StreamExt;
use kube::{
    api::Api,
    client::Client,
    runtime::{controller::Controller, watcher::Config},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;
use tracing_actix_web::TracingLogger;

mod controller;
mod finalizer;
mod gitlab;
mod namespace;
mod project;
mod project_crd;
mod rbacs;
mod repository;
mod secret;

use crate::gitlab::Gitlab;
use crate::project_crd::Project;

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
    //init dotenv
    dotenv::dotenv().ok();
    //set tracing log level based on env var
    let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let log_level = match log_level.as_str() {
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    //init tracing with env_logger
    tracing_subscriber::FmtSubscriber::builder()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_max_level(log_level)
        .json()
        .init();

    let kubernetes_client = Client::try_default()
        .await
        .expect("Failed to create client");

    let crd_api: Api<Project> = Api::all(kubernetes_client.clone());

    let gitlab_url = std::env::var("GITLAB_URL").expect("GITLAB_URL not set");
    let gitlab_token = std::env::var("GITLAB_TOKEN").expect("GITLAB_TOKEN not set");

    let gitlab = Gitlab::new(gitlab_url, gitlab_token);

    let context: Arc<controller::ContextData> = Arc::new(controller::ContextData {
        client: kubernetes_client.clone(),
        gitlab: gitlab.clone(),
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

    let controller = Controller::new(crd_api.clone(), Config::default().any_semantic())
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

    let _server = tokio::spawn(srv.run());

    let _contro = tokio::spawn(controller);

    let mut sig_term = signal(SignalKind::terminate())?;
    let mut sig_int = signal(SignalKind::interrupt())?;

    tokio::select! {
        _ = sig_term.recv() => log::info!("SIGTERM received"),
        _ = sig_int.recv() => log::info!("SIGINT received"),
    }

    Ok(())
}
