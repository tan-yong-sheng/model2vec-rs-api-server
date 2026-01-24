use anyhow::Result;
use moka::future::Cache;
use model2vec_rs::model::StaticModel;
use std::sync::Arc;
use std::hash::Hash;
use std::cmp::Eq;

/// Cache key for vectorization requests
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct VectorizeCacheKey {
    pub input: String,
}

impl VectorizeCacheKey {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.to_string(),
        }
    }
}

/// Wrapper for input that can be string or array
#[derive(Debug, Clone)]
pub enum TextInput {
    Single(String),
    Multiple(Vec<String>),
}

impl TextInput {
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            TextInput::Single(s) => vec![s.clone()],
            TextInput::Multiple(v) => v.clone(),
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        match self {
            TextInput::Single(_) => 1,
            TextInput::Multiple(v) => v.len(),
        }
    }
}

impl From<&str> for TextInput {
    fn from(s: &str) -> Self {
        TextInput::Single(s.to_string())
    }
}

impl From<String> for TextInput {
    fn from(s: String) -> Self {
        TextInput::Single(s)
    }
}

impl From<Vec<String>> for TextInput {
    fn from(v: Vec<String>) -> Self {
        TextInput::Multiple(v)
    }
}

/// The Model2Vec vectorizer with caching
#[derive(Clone)]
pub struct Model2VecVectorizer {
    model: Arc<StaticModel>,
    cache: Cache<VectorizeCacheKey, Vec<f32>>,
}

impl Model2VecVectorizer {
    /// Create a new vectorizer by loading model from directory
    pub async fn new(model_path: &str) -> Result<Self> {
        let model = Arc::new(StaticModel::from_pretrained(
            model_path,
            None,  // no HF token for local files
            None,  // use model's default normalize setting
            None,  // no subfolder
        )?);

        // Create cache with max 1024 entries and 600 second TTL
        let cache = Cache::builder()
            .max_capacity(1024)
            .time_to_live(std::time::Duration::from_secs(600))
            .build();

        Ok(Self { model, cache })
    }

    /// Vectorize text input
    pub async fn vectorize(&self, input: &TextInput) -> Vec<Vec<f32>> {
        let texts = input.to_vec();
        let mut results: Vec<Vec<f32>> = Vec::with_capacity(texts.len());

        for text in texts {
            // Check cache first
            let cache_key = VectorizeCacheKey::new(&text);

            if let Some(cached) = self.cache.get(&cache_key).await {
                results.push(cached);
                continue;
            }

            // Encode the text
            let embeddings = self.model.encode(&[text.clone()]);
            let embedding = embeddings.into_iter().next().unwrap_or_default();

            // Cache the result
            self.cache.insert(cache_key, embedding.clone()).await;

            results.push(embedding);
        }

        results
    }
}
