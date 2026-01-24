use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub model_name: String,
    pub alias_model_name: Option<String>,
    pub allowed_tokens: Vec<String>,
    pub port: u16,
    pub model_path: String,
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

        Self {
            model_name,
            alias_model_name,
            allowed_tokens,
            port,
            model_path: "./models".to_string(),
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
