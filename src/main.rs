use std::sync::Arc;
use dotenvy::dotenv;
use tracing_subscriber::fmt;
use anyhow::Result;
use model2vec_api::app;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    fmt::init();

    // Load environment variables from .env file
    dotenv().ok();

    tracing::info!("Starting Model2Vec API Server (Rust)");

    // Create shared application state
    let app_state = Arc::new(app::AppState::new().await?);
    let app = app::routes::create_router(app_state.clone());

    // Start the server
    let host = "0.0.0.0";
    let port = app_state.config.port;

    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}"))
        .await?;

    tracing::info!("Server listening on {}:{}", host, port);
    tracing::info!("Health checks: /.well-known/live, /.well-known/ready");
    tracing::info!("Embeddings endpoint: /v1/embeddings");
    tracing::info!("Models endpoint: /v1/models");

    axum::serve(listener, app)
        .await?;

    Ok(())
}
