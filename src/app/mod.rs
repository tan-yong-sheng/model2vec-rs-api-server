use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::RwLock;

use anyhow::Result;

use crate::{
    config::Config,
    vectorizer::{CacheSettings, InferenceSettings, LoadSettings, Model2VecVectorizer, Vectorizer},
};

pub mod models;
pub mod routes;
pub mod auth;

/// Application state shared across requests
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    vectorizer: Arc<RwLock<Option<Arc<dyn Vectorizer>>>>,
    last_request_time: Arc<AtomicU64>,
}

static VECTORIZER: RwLock<Option<Arc<dyn Vectorizer>>> = RwLock::const_new(None);

impl AppState {
    /// Create new application state
    pub async fn new() -> Result<Self> {
        let config = Arc::new(Config::from_env());
        // Initialize timestamp to 0 if lazy loading, otherwise current time
        let initial_timestamp = if config.lazy_load_model {
            0 // Will be set on first request
        } else {
            Self::current_timestamp()
        };
        let last_request_time = Arc::new(AtomicU64::new(initial_timestamp));

        // Initialize vectorizer based on lazy_load_model setting
        let vectorizer = if config.lazy_load_model {
            tracing::info!("Lazy loading enabled - model will load on first request");
            Arc::new(RwLock::new(None))
        } else {
            tracing::info!("Eager loading model at startup: {}", config.model_name);
            let start = std::time::Instant::now();
            let vec = Self::load_model(&config).await?;
            let elapsed = start.elapsed();
            tracing::info!("Model loaded in {:.2}s", elapsed.as_secs_f64());
            Arc::new(RwLock::new(Some(vec)))
        };

        let state = Self {
            config: config.clone(),
            vectorizer,
            last_request_time,
        };

        // Start background task for model unloading if enabled
        if config.model_unload_enabled {
            tracing::info!(
                "Model unloading enabled - idle timeout: {}s",
                config.model_unload_idle_timeout
            );
            state.clone().start_idle_monitor();
        }

        Ok(state)
    }

    /// Get current timestamp in seconds
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Load the model
    async fn load_model(config: &Config) -> Result<Arc<dyn Vectorizer>> {
        // Check static cache first
        {
            let guard = VECTORIZER.read().await;
            if let Some(vec) = guard.as_ref() {
                return Ok(vec.clone());
            }
        }

        // Load the model (write lock to prevent duplicate loads)
        let mut guard = VECTORIZER.write().await;
        
        // Double-check after acquiring write lock (another task may have loaded it)
        if let Some(vec) = guard.as_ref() {
            return Ok(vec.clone());
        }

        let load_settings = Self::load_settings(config);
        let inference_settings = Self::inference_settings(config);
        let cache_settings = Self::cache_settings(config);
        let vec: Arc<dyn Vectorizer> = Arc::new(
            Model2VecVectorizer::new(
                &config.model_name,
                load_settings,
                inference_settings,
                cache_settings,
            )
            .await?,
        );

        // Store in static cache
        *guard = Some(vec.clone());
        
        Ok(vec)
    }

    /// Get or initialize the vectorizer (supports lazy loading)
    pub async fn get_vectorizer(&self) -> Result<Arc<dyn Vectorizer>> {
        // Update last request time
        self.last_request_time.store(Self::current_timestamp(), Ordering::Relaxed);

        // Check if already loaded
        {
            let guard = self.vectorizer.read().await;
            if let Some(vec) = guard.as_ref() {
                return Ok(vec.clone());
            }
        }

        // Lazy load the model on first use
        tracing::info!("Loading model on demand: {}", self.config.model_name);
        let start = std::time::Instant::now();
        let vec = Self::load_model(&self.config).await?;
        let elapsed = start.elapsed();
        tracing::info!("Model loaded on demand in {:.2}s", elapsed.as_secs_f64());

        // Store in instance (write lock to prevent race)
        {
            let mut guard = self.vectorizer.write().await;
            // Check again in case another task loaded it
            if guard.is_none() {
                *guard = Some(vec.clone());
            }
        }

        Ok(vec)
    }

    /// Unload the vectorizer to free memory
    async fn unload_vectorizer(&self) -> bool {
        // Acquire write lock and check if model is loaded before unloading
        let mut instance_guard = self.vectorizer.write().await;
        let mut static_guard = VECTORIZER.write().await;
        
        // Only unload if both are actually loaded
        if instance_guard.is_none() && static_guard.is_none() {
            return false; // Already unloaded
        }
        
        tracing::info!("Unloading model to free memory");
        
        // Clear both storages
        *instance_guard = None;
        *static_guard = None;

        tracing::info!("Model unloaded successfully");
        true
    }

    /// Start background task to monitor idle time and unload model
    fn start_idle_monitor(self) {
        // Calculate check interval: max(timeout / 10, 10 seconds)
        let check_interval = std::cmp::max(
            self.config.model_unload_idle_timeout.div_ceil(10),
            10
        );
        
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(check_interval)).await;

                let last_request = self.last_request_time.load(Ordering::Relaxed);
                
                // Skip if no requests have been made yet (lazy loading not triggered)
                if last_request == 0 {
                    continue;
                }

                // Check if model should be unloaded
                let now = Self::current_timestamp();
                let idle_duration = now.saturating_sub(last_request);

                if idle_duration >= self.config.model_unload_idle_timeout {
                    let was_unloaded = self.unload_vectorizer().await;
                    if was_unloaded {
                        tracing::info!(
                            "Model was idle for {}s (threshold: {}s)",
                            idle_duration,
                            self.config.model_unload_idle_timeout
                        );
                    }
                }
            }
        });
    }

    pub async fn is_ready(&self) -> bool {
        {
            let guard = self.vectorizer.read().await;
            if guard.is_some() {
                return true;
            }
        }

        let guard = VECTORIZER.read().await;
        guard.is_some()
    }

    fn load_settings(config: &Config) -> LoadSettings {
        LoadSettings {
            max_retries: config.model_load_max_retries,
            retry_base: std::time::Duration::from_millis(config.model_load_retry_base_ms),
            retry_max: std::time::Duration::from_millis(config.model_load_retry_max_ms),
            timeout: std::time::Duration::from_secs(config.model_load_timeout_secs),
        }
    }

    fn inference_settings(config: &Config) -> InferenceSettings {
        InferenceSettings {
            max_retries: config.inference_max_retries,
            retry_base: std::time::Duration::from_millis(config.inference_retry_base_ms),
            retry_max: std::time::Duration::from_millis(config.inference_retry_max_ms),
            timeout: std::time::Duration::from_secs(config.request_timeout_secs),
        }
    }

    fn cache_settings(config: &Config) -> CacheSettings {
        CacheSettings {
            max_entries: config.embedding_cache_max_entries,
            ttl: std::time::Duration::from_secs(config.embedding_cache_ttl_secs),
        }
    }

    pub fn new_with_vectorizer(config: Config, vectorizer: Arc<dyn Vectorizer>) -> Self {
        let now = Self::current_timestamp();
        Self {
            config: Arc::new(config),
            vectorizer: Arc::new(RwLock::new(Some(vectorizer))),
            last_request_time: Arc::new(AtomicU64::new(now)),
        }
    }
}
