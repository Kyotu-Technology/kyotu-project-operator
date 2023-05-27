use actix_web::{get, web::Data, HttpRequest, HttpResponse, Responder};
pub use controller::State;
use prometheus::{Encoder, TextEncoder};
use serde::{Deserialize, Serialize};
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;
use tracing_actix_web::TracingLogger;

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

#[get("/metrics")]
async fn metrics(c: Data<State>, _req: HttpRequest) -> impl Responder {
    let metrics = c.metrics();
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    encoder.encode(&metrics, &mut buffer).unwrap();
    HttpResponse::Ok().body(buffer)
}

#[get("/")]
async fn index(c: Data<State>, _req: HttpRequest) -> impl Responder {
    let d = c.diagnostics().await;
    HttpResponse::Ok().json(&d)
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

    //init tracing
    tracing_subscriber::FmtSubscriber::builder()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_max_level(log_level)
        .json()
        .init();
    let state = State::default();
    let contro = tokio::spawn(controller::run(state.clone()));

    //start server for health check and metrics
    let srv = actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .app_data(Data::new(state.clone()))
            .wrap(TracingLogger::default())
            .service(index)
            .service(health)
            .service(metrics)
    })
    .bind("0.0.0.0:8080")
    .expect("Failed to bind to port 8080")
    .shutdown_timeout(5);

    let server = tokio::spawn(srv.run());

    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();

    tokio::select! {
        _ = sigterm.recv() => {
            info!("SIGTERM received");
        },
        _ = sigint.recv() => {
            info!("SIGINT received");
        },
        _ = server => {
            info!("Server stopped");
        },
        _ = contro => {
            info!("Controller stopped");
        }
    }

    Ok(())
}
