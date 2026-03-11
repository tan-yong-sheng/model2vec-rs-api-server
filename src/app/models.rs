use serde::{Deserialize, Serialize};
use validator::Validate;

/// OpenAI-compatible embedding request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct EmbeddingRequest {
    /// The input text(s) to embed - can be a string or array of strings
    #[serde(deserialize_with = "deserialize_input")]
    pub input: InputType,

    /// The model ID to use
    #[validate(length(min = 1, message = "model must not be empty"))]
    pub model: String,

    /// Optional encoding format: "float" or "base64"
    #[serde(default = "default_encoding_format")]
    pub encoding_format: String,

    /// Optional dimensions to truncate to
    #[serde(default)]
    #[validate(range(min = 1, message = "dimensions must be positive"))]
    pub dimensions: Option<usize>,

    /// Optional additional configuration
    #[serde(default)]
    #[allow(dead_code)]
    pub config: Option<VectorInputConfig>,
}

fn default_encoding_format() -> String {
    "float".to_string()
}

/// Custom deserializer for InputType that handles both string and array
fn deserialize_input<'de, D>(deserializer: D) -> Result<InputType, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if let Some(s) = value.as_str() {
        Ok(InputType::Single(s.to_string()))
    } else if let Some(arr) = value.as_array() {
        let mut strings = Vec::new();
        for (i, v) in arr.iter().enumerate() {
            let s = v.as_str().ok_or_else(|| {
                serde::de::Error::custom(format!("expected string at array index {i}"))
            })?;
            strings.push(s.to_string());
        }
        Ok(InputType::Multiple(strings))
    } else {
        Err(serde::de::Error::custom("input must be a string or array of strings"))
    }
}

/// Input can be a single string or array of strings
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum InputType {
    Single(String),
    Multiple(Vec<String>),
}

impl InputType {
    pub fn to_text_input(&self) -> crate::vectorizer::TextInput {
        match self {
            InputType::Single(s) => crate::vectorizer::TextInput::Single(s.clone()),
            InputType::Multiple(v) => crate::vectorizer::TextInput::Multiple(v.clone()),
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        match self {
            InputType::Single(_) => 1,
            InputType::Multiple(v) => v.len(),
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Optional config for embedding request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VectorInputConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pooling_strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
}

/// OpenAI-compatible embedding response
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingResponse {
    pub object: String,
    pub data: Vec<EmbeddingObject>,
    pub model: String,
    pub usage: Usage,
}

/// Individual embedding object
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingObject {
    pub object: String,
    pub index: usize,
    #[serde(serialize_with = "serialize_embedding")]
    pub embedding: EmbeddingValue,
}

/// Usage statistics
#[derive(Debug, Clone, Serialize)]
pub struct Usage {
    pub prompt_tokens: usize,
    pub total_tokens: usize,
}

/// OpenAI-compatible model list response
#[derive(Debug, Clone, Serialize)]
pub struct ModelListResponse {
    pub object: String,
    pub data: Vec<ModelObject>,
}

/// Individual model object
#[derive(Debug, Clone, Serialize)]
pub struct ModelObject {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
    #[serde(default)]
    pub permission: Vec<()>,
    pub root: String,
    #[serde(default)]
    pub parent: Option<String>,
}

/// Model metadata response
#[derive(Debug, Clone, Serialize)]
pub struct ModelMetadata {
    #[serde(default)]
    pub model_path: String,
    #[serde(default)]
    pub model_name: String,
}

/// Error response
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorBody {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub param: Option<String>,
    pub code: Option<String>,
}

impl ErrorResponse {
    pub fn invalid_request(message: impl Into<String>, param: Option<&str>) -> Self {
        Self {
            error: ErrorBody {
                message: message.into(),
                error_type: "invalid_request_error".to_string(),
                param: param.map(|p| p.to_string()),
                code: None,
            },
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            error: ErrorBody {
                message: message.into(),
                error_type: "authentication_error".to_string(),
                param: None,
                code: None,
            },
        }
    }

    pub fn server_error(message: impl Into<String>) -> Self {
        Self {
            error: ErrorBody {
                message: message.into(),
                error_type: "server_error".to_string(),
                param: None,
                code: None,
            },
        }
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self {
            error: ErrorBody {
                message: message.into(),
                error_type: "rate_limit_error".to_string(),
                param: None,
                code: None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum EmbeddingValue {
    Float(Vec<f32>),
    Base64(String),
}

// Custom serializer to handle both f32 and base64 encoding
fn serialize_embedding<S>(embedding: &EmbeddingValue, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    embedding.serialize(serializer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserialize_input_single() {
        let value = json!({
            "input": "hello",
            "model": "test-model"
        });
        let request: EmbeddingRequest = serde_json::from_value(value).expect("valid request");
        match request.input {
            InputType::Single(s) => assert_eq!(s, "hello"),
            _ => panic!("expected single input"),
        }
        assert_eq!(request.encoding_format, "float");
    }

    #[test]
    fn deserialize_input_multiple() {
        let value = json!({
            "input": ["a", "b"],
            "model": "test-model"
        });
        let request: EmbeddingRequest = serde_json::from_value(value).expect("valid request");
        match request.input {
            InputType::Multiple(v) => assert_eq!(v, vec!["a".to_string(), "b".to_string()]),
            _ => panic!("expected multiple input"),
        }
    }

    #[test]
    fn deserialize_input_invalid_array() {
        let value = json!({
            "input": ["ok", 1],
            "model": "test-model"
        });
        let err = serde_json::from_value::<EmbeddingRequest>(value).unwrap_err();
        assert!(err.to_string().contains("expected string"));
    }

    #[test]
    fn input_type_to_text_input() {
        let single = InputType::Single("hi".to_string()).to_text_input();
        match single {
            crate::vectorizer::TextInput::Single(s) => assert_eq!(s, "hi"),
            _ => panic!("expected single"),
        }

        let multiple = InputType::Multiple(vec!["a".to_string(), "b".to_string()]).to_text_input();
        match multiple {
            crate::vectorizer::TextInput::Multiple(v) => assert_eq!(v, vec!["a".to_string(), "b".to_string()]),
            _ => panic!("expected multiple"),
        }
    }

    #[test]
    fn error_response_builders() {
        let err = ErrorResponse::invalid_request("bad", Some("input"));
        assert_eq!(err.error.error_type, "invalid_request_error");
        assert_eq!(err.error.param.as_deref(), Some("input"));

        let err = ErrorResponse::unauthorized("nope");
        assert_eq!(err.error.error_type, "authentication_error");

        let err = ErrorResponse::server_error("oops");
        assert_eq!(err.error.error_type, "server_error");

        let err = ErrorResponse::rate_limited("slow down");
        assert_eq!(err.error.error_type, "rate_limit_error");
    }
}
