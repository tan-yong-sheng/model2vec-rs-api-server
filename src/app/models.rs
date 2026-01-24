use serde::{Deserialize, Serialize};
use validator::Validate;

/// OpenAI-compatible embedding request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct EmbeddingRequest {
    /// The input text(s) to embed - can be a string or array of strings
    #[serde(deserialize_with = "deserialize_input")]
    pub input: InputType,

    /// The model ID to use
    pub model: String,

    /// Optional encoding format: "float" or "base64"
    #[serde(default = "default_encoding_format")]
    pub encoding_format: String,

    /// Optional dimensions to truncate to
    #[serde(default)]
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
                serde::de::Error::custom(format!("expected string at array index {}", i))
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
    pub embedding: Vec<f32>,
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
    pub error: String,
}

// Custom serializer to handle both f32 and base64 encoding
fn serialize_embedding<S>(embedding: &[f32], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    embedding.serialize(serializer)
}
