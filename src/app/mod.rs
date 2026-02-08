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
    pub vectorizer: Arc<Option<Arc<Model2VecVectorizer>>>,
}

static VECTORIZER: OnceCell<Arc<Model2VecVectorizer>> = OnceCell::const_new();

impl AppState {
    /// Create new application state
    pub async fn new() -> Self {
        let config = Arc::new(Config::from_env());

        // Initialize vectorizer based on lazy_load_model setting
        let vectorizer = if config.lazy_load_model {
            tracing::info!("Lazy loading enabled - model will load on first request");
            // Don't load the model yet - it will be loaded on first use
            Arc::new(None)
        } else {
            tracing::info!("Eager loading model at startup: {}", config.model_name);
            let start = std::time::Instant::now();
            let vec = VECTORIZER
                .get_or_init(|| async {
                    Arc::new(
                        Model2VecVectorizer::new(&config.model_name)
                            .await
                            .expect("Failed to load model"),
                    )
                })
                .await
                .clone();
            let elapsed = start.elapsed();
            tracing::info!("Model loaded in {:.2}s", elapsed.as_secs_f64());
            Arc::new(Some(vec))
        };

        Self {
            config,
            vectorizer,
        }
    }

    /// Get or initialize the vectorizer (supports lazy loading)
    pub async fn get_vectorizer(&self) -> Arc<Model2VecVectorizer> {
        if let Some(vec) = self.vectorizer.as_ref() {
            return vec.clone();
        }

        // Lazy load the model on first use
        tracing::info!("Loading model on demand: {}", self.config.model_name);
        let start = std::time::Instant::now();
        let vec = VECTORIZER
            .get_or_init(|| async {
                Arc::new(
                    Model2VecVectorizer::new(&self.config.model_name)
                        .await
                        .expect("Failed to load model"),
                )
            })
            .await
            .clone();
        let elapsed = start.elapsed();
        tracing::info!("Model loaded on demand in {:.2}s", elapsed.as_secs_f64());
        vec
    }
}
