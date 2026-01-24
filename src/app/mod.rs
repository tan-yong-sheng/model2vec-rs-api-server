use std::sync::Arc;

use tokio::sync::OnceCell;

use crate::{config::Config, vectorizer::Model2VecVectorizer};

pub mod models;
pub mod routes;
pub mod auth;

/// Application state shared across requests
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub vectorizer: Arc<Model2VecVectorizer>,
}

static VECTORIZER: OnceCell<Arc<Model2VecVectorizer>> = OnceCell::const_new();

impl AppState {
    /// Create new application state
    pub async fn new() -> Self {
        let config = Arc::new(Config::from_env());

        // Initialize vectorizer (only once)
        let vectorizer = VECTORIZER
            .get_or_init(|| async {
                Arc::new(
                    Model2VecVectorizer::new(&config.model_name)
                        .await
                        .expect("Failed to load model"),
                )
            })
            .await
            .clone();

        Self {
            config,
            vectorizer,
        }
    }
}
