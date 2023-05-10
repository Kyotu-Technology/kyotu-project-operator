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

    let repo_url = std::env::var("ARGO_REPO").expect("ARGO_REPO not set");
    let repo_root = std::env::var("REPO_ROOT").expect("REPO_ROOT not set");
    let repo_branch = std::env::var("REPO_BRANCH").expect("REPO_BRANCH not set");

    let context: Arc<controller::ContextData> = Arc::new(controller::ContextData {
        client: kubernetes_client.clone(),
    });

    //start server for health check
    let srv = actix_web::HttpServer::new(|| {
        actix_web::App::new()
            .wrap(TracingLogger::default())
            .service(health)
    })
    .bind(("127.0.0.1", 8080))
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

    //clone repo

    //remove repo_root if it exists
    if Path::new(&repo_root).exists() {
        std::fs::remove_dir_all(&repo_root).expect("Could not remove repo_root");
    }

    println!("Cloning {} into {}", repo_url, repo_root);

    let mut builder = RepoBuilder::new();
    let mut callbacks = RemoteCallbacks::new();
    let mut fetch_options = FetchOptions::new();

    callbacks.credentials(repository::credentials_cb);
    //callbacks.transfer_progress(|ref progress| repository::transfer_progress_cb(progress));

    fetch_options.remote_callbacks(callbacks);
    builder.fetch_options(fetch_options);
    builder.branch(&repo_branch);

    let repo = builder
        .clone(&repo_url, Path::new(&repo_root))
        .expect("Could not clone repo");
    tokio::join!(controller, srv.run()).1?;
    Ok(())
}

//
//match create_project("test", Path::new(&repo_root)) {
//    Ok(response) => log::info!("{}", response),
//    Err(e) => log::error!("Error: {:?}", e)
//}
//
//match delete_project("test", Path::new(&repo_root)) {
//    Ok(response) => log::info!("{}", response),
//    Err(e) => log::error!("Error: {:?}", e)
//}
//
//create commit
//let mut index = repo.index().expect("Could not get index");
//let oid = index.write_tree().expect("Could not write tree");
//let tree = repo.find_tree(oid).expect("Could not find tree");
//let sig = repo.signature().expect("Could not get signature");
//let parent_commit = repo.find_commit(repo.head().expect("Could not get head").target().expect("Could not get head target")).expect("Could not find commit");
//repo.commit(Some("HEAD"), &sig, &sig, "Test commit", &tree, &[&parent_commit]).expect("Could not create commit");

//push commit
//let mut remote = repo.find_remote("origin").expect("Could not find remote");
//let mut push_options = git2::PushOptions::new();
//let mut push_callbacks = RemoteCallbacks::new();
//push_callbacks.credentials(credentials_cb);
//push_callbacks.transfer_progress(|ref progress| transfer_progress_cb(progress));
//push_options.remote_callbacks(push_callbacks);
//remote.push(&[&format!("refs/heads/{}", repo_branch)], Some(&mut push_options)).expect("Could not push to remote");
