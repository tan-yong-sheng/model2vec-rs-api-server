use crate::app::models::*;
use crate::app::auth::{auth_middleware, AuthState};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json,
    Router,
};
use std::sync::Arc;

/// Wrapper enum for responses - allows different response types
enum ApiResponse {
    Error(StatusCode, ErrorResponse),
    Metadata(ModelMetadata),
    Models(ModelListResponse),
    Embeddings(EmbeddingResponse),
}

impl IntoResponse for ApiResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            ApiResponse::Error(status, err) => (status, Json(err)).into_response(),
            ApiResponse::Metadata(m) => (StatusCode::OK, Json(m)).into_response(),
            ApiResponse::Models(m) => (StatusCode::OK, Json(m)).into_response(),
            ApiResponse::Embeddings(e) => (StatusCode::OK, Json(e)).into_response(),
        }
    }
}

/// Health check: live
pub async fn live() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

/// Health check: ready
pub async fn ready() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

/// Get model metadata
pub async fn meta(
    State(state): State<Arc<crate::app::AppState>>,
) -> impl IntoResponse {
    let config = &state.config;
    let model_path = &config.model_path;

    // Try to read model config.json if it exists
    if std::path::Path::new(format!("{}/config.json", model_path).as_str()).exists() {
        let config_path = format!("{}/config.json", model_path);
        if let Ok(content) = std::fs::read_to_string(config_path) {
            if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(name) = config_json.get("model_name").and_then(|v| v.as_str()) {
                    return ApiResponse::Metadata(ModelMetadata {
                        model_path: model_path.clone(),
                        model_name: name.to_string(),
                    });
                }
            }
        }
    }

    ApiResponse::Metadata(ModelMetadata {
        model_path: model_path.clone(),
        model_name: config.model_name.clone(),
    })
}

/// List available models
pub async fn list_models(
    State(state): State<Arc<crate::app::AppState>>,
) -> impl IntoResponse {
    let config = &state.config;
    let model_display_name = &config.model_name;
    let alias = config.alias_model_name.as_ref();

    let mut models = vec![ModelObject {
        id: model_display_name.clone(),
        object: "model".to_string(),
        created: 1700000000,
        owned_by: "minishlab".to_string(),
        permission: vec![],
        root: model_display_name.clone(),
        parent: None,
    }];

    // Add alias as separate model if set and different from main model
    if let Some(alias_name) = alias {
        if alias_name != model_display_name {
            models.push(ModelObject {
                id: alias_name.clone(),
                object: "model".to_string(),
                created: 1700000000,
                owned_by: "minishlab".to_string(),
                permission: vec![],
                root: alias_name.clone(),
                parent: Some(model_display_name.clone()),
            });
        }
    }

    ApiResponse::Models(ModelListResponse {
        object: "list".to_string(),
        data: models,
    })
}

/// Create embeddings
pub async fn create_embeddings(
    State(state): State<Arc<crate::app::AppState>>,
    Json(request): Json<EmbeddingRequest>,
) -> impl IntoResponse {
    let config = &state.config;
    let available_model = &config.model_name;
    let alias = config.alias_model_name.as_ref();

    if &request.model != available_model && alias.map(|a| &request.model != a).unwrap_or(true) {
        return ApiResponse::Error(
            StatusCode::BAD_REQUEST,
            ErrorResponse {
                error: format!(
                    "Model '{}' not found. Available model: '{}'",
                    request.model, available_model
                ),
            },
        );
    }

    // Validate encoding format
    if request.encoding_format != "float" && request.encoding_format != "base64" {
        return ApiResponse::Error(
            StatusCode::BAD_REQUEST,
            ErrorResponse {
                error: "encoding_format must be 'float' or 'base64'".to_string(),
            },
        );
    }

    // Vectorize
    let input = request.input.to_text_input();
    let embeddings = state.vectorizer.vectorize(&input).await;

    // Handle dimensions truncation
    let embeddings: Vec<Vec<f32>> = if let Some(dims) = request.dimensions {
        if dims > 0 {
            embeddings
                .into_iter()
                .map(|e| e.into_iter().take(dims).collect())
                .collect()
        } else {
            embeddings
        }
    } else {
        embeddings
    };

    // Build response
    let data: Vec<EmbeddingObject> = embeddings
        .iter()
        .enumerate()
        .map(|(i, emb)| EmbeddingObject {
            object: "embedding".to_string(),
            index: i,
            embedding: emb.clone(),
        })
        .collect();

    // Calculate approximate token usage
    let total_tokens: usize = match &request.input {
        InputType::Single(s) => s.split_whitespace().count(),
        InputType::Multiple(v) => v.iter().map(|t| t.split_whitespace().count()).sum(),
    };

    ApiResponse::Embeddings(EmbeddingResponse {
        object: "list".to_string(),
        data,
        model: request.model,
        usage: Usage {
            prompt_tokens: total_tokens,
            total_tokens,
        },
    })
}

/// Create router with optional auth middleware
pub fn create_router(app_state: Arc<crate::app::AppState>) -> Router {
    let mut router = Router::new()
        // Health endpoints (no auth)
        .route("/.well-known/live", get(live))
        .route("/.well-known/ready", get(ready))
        // Meta endpoint
        .route("/meta", get(meta))
        // OpenAI-compatible endpoints
        .route("/v1/models", get(list_models))
        .route("/models", get(list_models))
        .route("/v1/embeddings", post(create_embeddings))
        .route("/embeddings", post(create_embeddings))
        .with_state(app_state.clone());

    // Add auth middleware if auth is enabled
    if app_state.config.is_auth_enabled() {
        let auth_state = AuthState::new(app_state.config.clone());
        router = router.layer(axum::middleware::from_fn(move |req, next| {
            auth_middleware(auth_state.clone(), req, next)
        }));
    }

    router
}
