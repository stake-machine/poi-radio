use axum::{extract::Extension, middleware, routing::get, Router, Server};

use std::future::ready;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use tracing::info;

use crate::attestation::LocalAttestationsMap;
use crate::server::model::{build_schema, POIRadioContext};
use crate::server::observability::{create_prometheus_recorder, track_metrics};
use crate::server::routes::{graphql_handler, graphql_playground, health};
use crate::{shutdown_signal, CONFIG};

pub mod model;
pub mod observability;
pub mod routes;

/// This function starts a API server at the configured server_host and server_port
pub async fn run_server(
    running_program: Arc<AtomicBool>,
    local_attestations: Arc<AsyncMutex<LocalAttestationsMap>>,
) {
    info!("Initializing HTTP server");
    let host = CONFIG
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .server_host
        .clone()
        .unwrap_or(String::from("0.0.0.0"));
    let port = CONFIG.get().unwrap().lock().unwrap().server_port.unwrap();

    let context = Arc::new(POIRadioContext::init(Arc::clone(&local_attestations)).await);

    let schema = build_schema(Arc::clone(&context)).await;

    let prometheus_recorder = create_prometheus_recorder();

    info!("API Service starting at {host}:{port}");

    let app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(move || ready(prometheus_recorder.render())))
        .route(
            "/api/v1/graphql",
            get(graphql_playground).post(graphql_handler),
        )
        .route_layer(middleware::from_fn(track_metrics))
        .layer(Extension(schema))
        .layer(Extension(context));
    let addr =
        SocketAddr::from_str(&format!("{}:{}", host, port)).expect("Start Prometheus metrics");

    Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal(running_program))
        .await
        .unwrap();
}
