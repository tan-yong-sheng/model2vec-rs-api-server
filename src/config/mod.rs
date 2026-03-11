use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub model_name: String,
    pub alias_model_name: Option<String>,
    pub allowed_tokens: Vec<String>,
    pub port: u16,
    pub lazy_load_model: bool,
    pub model_unload_enabled: bool,
    pub model_unload_idle_timeout: u64,
    pub request_timeout_secs: u64,
    pub request_body_limit_bytes: usize,
    pub max_input_items: usize,
    pub max_input_chars: usize,
    pub max_total_chars: usize,
    pub concurrency_limit: usize,
    pub model_load_max_retries: u32,
    pub model_load_retry_base_ms: u64,
    pub model_load_retry_max_ms: u64,
    pub model_load_timeout_secs: u64,
    pub inference_max_retries: u32,
    pub inference_retry_base_ms: u64,
    pub inference_retry_max_ms: u64,
    pub embedding_cache_max_entries: u64,
    pub embedding_cache_ttl_secs: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let model_name = env::var("MODEL_NAME").unwrap_or_else(|_| "minishlab/potion-base-8M".to_string());
        let alias_model_name = env::var("ALIAS_MODEL_NAME").ok();
        let allowed_tokens = env::var("AUTHENTICATION_ALLOWED_TOKENS")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
            .unwrap_or_default();
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
        let lazy_load_model = env::var("LAZY_LOAD_MODEL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
        let model_unload_enabled = env::var("MODEL_UNLOAD_ENABLED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
        let model_unload_idle_timeout = env::var("MODEL_UNLOAD_IDLE_TIMEOUT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1800); // 30 minutes default
        let request_timeout_secs = env::var("REQUEST_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        let request_body_limit_bytes = env::var("REQUEST_BODY_LIMIT_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2_000_000);
        let max_input_items = env::var("MAX_INPUT_ITEMS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(512);
        let max_input_chars = env::var("MAX_INPUT_CHARS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8192);
        let max_total_chars = env::var("MAX_TOTAL_CHARS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(200_000);
        let concurrency_limit = env::var("CONCURRENCY_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(64);
        let model_load_max_retries = env::var("MODEL_LOAD_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        let model_load_retry_base_ms = env::var("MODEL_LOAD_RETRY_BASE_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(200);
        let model_load_retry_max_ms = env::var("MODEL_LOAD_RETRY_MAX_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5_000);
        let model_load_timeout_secs = env::var("MODEL_LOAD_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(120);
        let inference_max_retries = env::var("INFERENCE_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);
        let inference_retry_base_ms = env::var("INFERENCE_RETRY_BASE_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);
        let inference_retry_max_ms = env::var("INFERENCE_RETRY_MAX_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(500);
        let embedding_cache_max_entries = env::var("EMBEDDING_CACHE_MAX_ENTRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1024);
        let embedding_cache_ttl_secs = env::var("EMBEDDING_CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(600);

        Self {
            model_name,
            alias_model_name,
            allowed_tokens,
            port,
            lazy_load_model,
            model_unload_enabled,
            model_unload_idle_timeout,
            request_timeout_secs,
            request_body_limit_bytes,
            max_input_items,
            max_input_chars,
            max_total_chars,
            concurrency_limit,
            model_load_max_retries,
            model_load_retry_base_ms,
            model_load_retry_max_ms,
            model_load_timeout_secs,
            inference_max_retries,
            inference_retry_base_ms,
            inference_retry_max_ms,
            embedding_cache_max_entries,
            embedding_cache_ttl_secs,
        }
    }

    pub fn is_auth_enabled(&self) -> bool {
        !self.allowed_tokens.is_empty()
    }

    pub fn is_valid_token(&self, token: &str) -> bool {
        if !self.is_auth_enabled() {
            return true;
        }
        self.allowed_tokens.iter().any(|t| t == token)
    }
}
