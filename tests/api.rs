use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use model2vec_api::app::routes::create_router;
use model2vec_api::app::AppState;
use model2vec_api::config::Config;
use model2vec_api::vectorizer::{TextInput, Vectorizer};
use serde_json::json;
use tower::ServiceExt;
use base64::Engine;

struct MockVectorizer;

#[async_trait::async_trait]
impl Vectorizer for MockVectorizer {
    async fn vectorize(&self, input: &TextInput) -> anyhow::Result<Vec<Vec<f32>>> {
        let count = match input {
            TextInput::Single(_) => 1,
            TextInput::Multiple(v) => v.len(),
        };
        Ok(vec![vec![1.0, 2.0, 3.0]; count])
    }
}

fn test_config() -> Config {
    Config {
        model_name: "minishlab/potion-base-8M".to_string(),
        alias_model_name: None,
        allowed_tokens: vec![],
        port: 8080,
        lazy_load_model: false,
        model_unload_enabled: false,
        model_unload_idle_timeout: 1800,
        request_timeout_secs: 30,
        request_body_limit_bytes: 2_000_000,
        max_input_items: 10,
        max_input_chars: 1000,
        max_total_chars: 10_000,
        concurrency_limit: 64,
        model_load_max_retries: 1,
        model_load_retry_base_ms: 1,
        model_load_retry_max_ms: 10,
        model_load_timeout_secs: 1,
        inference_max_retries: 1,
        inference_retry_base_ms: 1,
        inference_retry_max_ms: 10,
        embedding_cache_max_entries: 10,
        embedding_cache_ttl_secs: 60,
    }
}

fn test_app() -> axum::Router {
    let state = AppState::new_with_vectorizer(test_config(), Arc::new(MockVectorizer));
    create_router(Arc::new(state))
}

#[tokio::test]
async fn root_returns_status() {
    let app = test_app();
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1_000_000).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["service"], "model2vec-api");
    assert_eq!(value["status"], "ready");
}

#[tokio::test]
async fn embeddings_float_success() {
    let app = test_app();
    let payload = json!({
        "input": "hello",
        "model": "minishlab/potion-base-8M",
        "encoding_format": "float"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/embeddings")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1_000_000).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["data"][0]["embedding"][0], 1.0);
}

#[tokio::test]
async fn embeddings_base64_success() {
    let app = test_app();
    let payload = json!({
        "input": "hello",
        "model": "minishlab/potion-base-8M",
        "encoding_format": "base64"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/embeddings")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1_000_000).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let encoded = value["data"][0]["embedding"].as_str().unwrap();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("base64 decode");
    assert_eq!(decoded.len(), 3 * 4);
}

#[tokio::test]
async fn embeddings_invalid_model() {
    let app = test_app();
    let payload = json!({
        "input": "hello",
        "model": "unknown-model"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/embeddings")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
