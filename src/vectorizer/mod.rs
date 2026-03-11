use anyhow::{anyhow, Result};
use moka::future::Cache;
use model2vec_rs::model::StaticModel;
use std::cmp::Eq;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use tokio::task::spawn_blocking;
use tokio::time::timeout;

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

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
    inference: InferenceSettings,
}

#[async_trait]
pub trait Vectorizer: Send + Sync {
    async fn vectorize(&self, input: &TextInput) -> Result<Vec<Vec<f32>>>;
}

impl Model2VecVectorizer {
    /// Create a new vectorizer by loading model with retry/backoff.
    pub async fn new(
        model_name: &str,
        load: LoadSettings,
        inference: InferenceSettings,
        cache: CacheSettings,
    ) -> Result<Self> {
        let model = load_model_with_retry(model_name, &load).await?;

        let cache = Cache::builder()
            .max_capacity(cache.max_entries)
            .time_to_live(cache.ttl)
            .build();

        Ok(Self {
            model,
            cache,
            inference,
        })
    }

    /// Vectorize text input
    pub async fn vectorize(&self, input: &TextInput) -> Result<Vec<Vec<f32>>> {
        let texts = input.to_vec();
        let mut results: Vec<Option<Vec<f32>>> = vec![None; texts.len()];
        let mut missing_texts: Vec<String> = Vec::new();
        let mut missing_indices: Vec<usize> = Vec::new();

        for (idx, text) in texts.iter().enumerate() {
            let cache_key = VectorizeCacheKey::new(text);
            if let Some(cached) = self.cache.get(&cache_key).await {
                results[idx] = Some(cached);
            } else {
                missing_texts.push(text.clone());
                missing_indices.push(idx);
            }
        }

        if !missing_texts.is_empty() {
            let embeddings = self.encode_with_retry(&missing_texts).await?;
            if embeddings.len() != missing_texts.len() {
                return Err(anyhow!(
                    "model returned {} embeddings for {} inputs",
                    embeddings.len(),
                    missing_texts.len()
                ));
            }

            for (idx, embedding) in missing_indices.into_iter().zip(embeddings.into_iter()) {
                let cache_key = VectorizeCacheKey::new(&texts[idx]);
                self.cache.insert(cache_key, embedding.clone()).await;
                results[idx] = Some(embedding);
            }
        }

        Ok(results
            .into_iter()
            .map(|item| item.unwrap_or_default())
            .collect())
    }

    async fn encode_with_retry(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut attempt: u32 = 0;
        let mut delay = self.inference.retry_base;
        let texts_vec = texts.to_vec();

        loop {
            let model = self.model.clone();
            let batch = texts_vec.clone();
            let handle = spawn_blocking(move || model.encode(&batch));
            let result = timeout(self.inference.timeout, handle).await;

            match result {
                Ok(Ok(embeddings)) => return Ok(embeddings),
                Ok(Err(err)) => {
                    let err = anyhow!("inference task failed: {err}");
                    if attempt >= self.inference.max_retries {
                        return Err(err);
                    }
                }
                Err(_) => {
                    let err = anyhow!(
                        "inference timed out after {}s",
                        self.inference.timeout.as_secs()
                    );
                    if attempt >= self.inference.max_retries {
                        return Err(err);
                    }
                }
            }

            tokio::time::sleep(delay).await;
            delay = std::cmp::min(delay.saturating_mul(2), self.inference.retry_max);
            attempt += 1;
        }
    }
}

#[async_trait]
impl Vectorizer for Model2VecVectorizer {
    async fn vectorize(&self, input: &TextInput) -> Result<Vec<Vec<f32>>> {
        Model2VecVectorizer::vectorize(self, input).await
    }
}

#[derive(Debug, Clone)]
pub struct LoadSettings {
    pub max_retries: u32,
    pub retry_base: Duration,
    pub retry_max: Duration,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct InferenceSettings {
    pub max_retries: u32,
    pub retry_base: Duration,
    pub retry_max: Duration,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct CacheSettings {
    pub max_entries: u64,
    pub ttl: Duration,
}

async fn load_model_with_retry(model_name: &str, settings: &LoadSettings) -> Result<Arc<StaticModel>> {
    let mut attempt: u32 = 0;
    let mut delay = settings.retry_base;

    loop {
        let model_name = model_name.to_string();
        let handle = spawn_blocking(move || {
            StaticModel::from_pretrained(
                &model_name,
                None, // no HF token for local files
                None, // use model's default normalize setting
                None, // no subfolder
            )
        });
        let result = timeout(settings.timeout, handle).await;

        match result {
            Ok(Ok(Ok(model))) => return Ok(Arc::new(model)),
            Ok(Ok(Err(err))) => {
                let err = anyhow!("model load failed: {err}");
                if attempt >= settings.max_retries {
                    return Err(err);
                }
            }
            Ok(Err(err)) => {
                let err = anyhow!("model load task failed: {err}");
                if attempt >= settings.max_retries {
                    return Err(err);
                }
            }
            Err(_) => {
                let err = anyhow!(
                    "model load timed out after {}s",
                    settings.timeout.as_secs()
                );
                if attempt >= settings.max_retries {
                    return Err(err);
                }
            }
        }

        tokio::time::sleep(delay).await;
        delay = std::cmp::min(delay.saturating_mul(2), settings.retry_max);
        attempt += 1;
    }
}
