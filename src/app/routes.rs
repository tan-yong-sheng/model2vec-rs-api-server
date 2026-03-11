use crate::app::auth::{auth_middleware, AuthState};
use crate::app::models::*;
use crate::config::Config;
use axum::{
    error_handling::HandleErrorLayer,
    extract::DefaultBodyLimit,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json,
    Router,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use std::sync::Arc;
use std::time::Duration;
use tower::limit::ConcurrencyLimitLayer;
use tower::load_shed::LoadShedLayer;
use tower::timeout::TimeoutLayer;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use axum::BoxError;
use validator::Validate;

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
pub async fn ready(
    State(state): State<Arc<crate::app::AppState>>,
) -> axum::response::Response {
    if state.is_ready().await {
        StatusCode::NO_CONTENT.into_response()
    } else {
        ApiResponse::Error(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorResponse::server_error("Model not loaded"),
        )
        .into_response()
    }
}

/// Get model metadata
pub async fn meta(
    State(state): State<Arc<crate::app::AppState>>,
) -> impl IntoResponse {
    let config = &state.config;

    ApiResponse::Metadata(ModelMetadata {
        model_path: "".to_string(),
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
            ErrorResponse::invalid_request(
                format!(
                    "Model '{}' not found. Available model: '{}'",
                    request.model, available_model
                ),
                Some("model"),
            ),
        );
    }

    if let Err(err) = validate_embedding_request(&request, config) {
        return err;
    }

    // Vectorize
    let input = request.input.to_text_input();
    let vectorizer = match state.get_vectorizer().await {
        Ok(vec) => vec,
        Err(err) => {
            tracing::error!("Model unavailable: {}", err);
            return ApiResponse::Error(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorResponse::server_error("Model unavailable"),
            );
        }
    };
    let embeddings = match vectorizer.vectorize(&input).await {
        Ok(embeddings) => embeddings,
        Err(err) => {
            tracing::error!("Inference failed: {}", err);
            return ApiResponse::Error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorResponse::server_error("Inference failed"),
            );
        }
    };

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
            embedding: if request.encoding_format == "base64" {
                EmbeddingValue::Base64(encode_embedding_base64(emb))
            } else {
                EmbeddingValue::Float(emb.clone())
            },
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
    let middleware = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(HandleErrorLayer::new(handle_middleware_error))
        .layer(TimeoutLayer::new(Duration::from_secs(
            app_state.config.request_timeout_secs,
        )))
        .layer(LoadShedLayer::new())
        .layer(ConcurrencyLimitLayer::new(
            app_state.config.concurrency_limit,
        ));

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
        .with_state(app_state.clone())
        .layer(DefaultBodyLimit::max(
            app_state.config.request_body_limit_bytes,
        ))
        .layer(middleware);

    // Add auth middleware if auth is enabled
    if app_state.config.is_auth_enabled() {
        let auth_state = AuthState::new(app_state.config.clone());
        router = router.layer(axum::middleware::from_fn(move |req, next| {
            auth_middleware(auth_state.clone(), req, next)
        }));
    }

    router
}

fn validate_embedding_request(
    request: &EmbeddingRequest,
    config: &Config,
) -> Result<(), ApiResponse> {
    if let Err(errors) = request.validate() {
        let (param, message) = first_validation_error(&errors);
        return Err(ApiResponse::Error(
            StatusCode::BAD_REQUEST,
            ErrorResponse::invalid_request(message, param.as_deref()),
        ));
    }

    if request.encoding_format != "float" && request.encoding_format != "base64" {
        return Err(ApiResponse::Error(
            StatusCode::BAD_REQUEST,
            ErrorResponse::invalid_request(
                "encoding_format must be 'float' or 'base64'",
                Some("encoding_format"),
            ),
        ));
    }

    let (count, max_chars, total_chars, has_empty) = input_metrics(&request.input);
    if count == 0 {
        return Err(ApiResponse::Error(
            StatusCode::BAD_REQUEST,
            ErrorResponse::invalid_request("input must not be empty", Some("input")),
        ));
    }
    if count > config.max_input_items {
        return Err(ApiResponse::Error(
            StatusCode::BAD_REQUEST,
            ErrorResponse::invalid_request(
                format!(
                    "input array has {} items; max is {}",
                    count, config.max_input_items
                ),
                Some("input"),
            ),
        ));
    }
    if max_chars > config.max_input_chars {
        return Err(ApiResponse::Error(
            StatusCode::BAD_REQUEST,
            ErrorResponse::invalid_request(
                format!(
                    "input item has {} characters; max is {}",
                    max_chars, config.max_input_chars
                ),
                Some("input"),
            ),
        ));
    }
    if total_chars > config.max_total_chars {
        return Err(ApiResponse::Error(
            StatusCode::BAD_REQUEST,
            ErrorResponse::invalid_request(
                format!(
                    "total input has {} characters; max is {}",
                    total_chars, config.max_total_chars
                ),
                Some("input"),
            ),
        ));
    }
    if has_empty {
        return Err(ApiResponse::Error(
            StatusCode::BAD_REQUEST,
            ErrorResponse::invalid_request("input strings must not be empty", Some("input")),
        ));
    }

    Ok(())
}

fn first_validation_error(errors: &validator::ValidationErrors) -> (Option<String>, String) {
    for (field, field_errors) in errors.field_errors() {
        if let Some(error) = field_errors.first() {
            let message = error
                .message
                .as_ref()
                .map(|m| m.to_string())
                .unwrap_or_else(|| format!("{} is invalid", field));
            return (Some(field.to_string()), message);
        }
    }
    (None, "invalid request".to_string())
}

fn input_metrics(input: &InputType) -> (usize, usize, usize, bool) {
    match input {
        InputType::Single(s) => {
            let len = s.chars().count();
            (1, len, len, s.is_empty())
        }
        InputType::Multiple(values) => {
            let mut total: usize = 0;
            let mut max = 0;
            let mut has_empty = false;
            for v in values {
                let len = v.chars().count();
                if v.is_empty() {
                    has_empty = true;
                }
                total = total.saturating_add(len);
                if len > max {
                    max = len;
                }
            }
            (values.len(), max, total, has_empty)
        }
    }
}

fn encode_embedding_base64(embedding: &[f32]) -> String {
    let mut bytes = Vec::with_capacity(embedding.len() * 4);
    for value in embedding {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    STANDARD.encode(bytes)
}

async fn handle_middleware_error(err: BoxError) -> impl IntoResponse {
    if err.is::<tower::timeout::error::Elapsed>() {
        return (
            StatusCode::GATEWAY_TIMEOUT,
            Json(ErrorResponse::server_error("Request timed out")),
        );
    }
    if err.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse::rate_limited(
                "Service overloaded, try again later",
            )),
        );
    }
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse::server_error("Internal server error")),
    )
}
