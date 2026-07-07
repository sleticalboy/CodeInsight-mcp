use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

pub const PROVIDER_ENV: &str = "CODEINSIGHT_EMBEDDING_PROVIDER";
pub const LOCAL_HASH_PROVIDER: &str = "local-hash";
pub const LOCAL_HASH_MODEL: &str = "local-hash-v1";
pub const SUPPORTED_PROVIDER_NAMES: &[&str] = &[LOCAL_HASH_PROVIDER, "local", "disabled", "none"];
const LOCAL_HASH_DIMENSIONS: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    pub values: Vec<f32>,
}

pub trait EmbeddingProvider {
    fn provider_name(&self) -> &'static str;
    fn model_name(&self) -> &'static str;
    fn is_configured(&self) -> bool {
        true
    }
    fn embed(&self, inputs: &[String]) -> Result<Vec<Embedding>>;
}

#[derive(Debug, Default)]
pub struct DisabledEmbeddingProvider;

impl EmbeddingProvider for DisabledEmbeddingProvider {
    fn provider_name(&self) -> &'static str {
        "disabled"
    }

    fn model_name(&self) -> &'static str {
        "disabled"
    }

    fn is_configured(&self) -> bool {
        false
    }

    fn embed(&self, _inputs: &[String]) -> Result<Vec<Embedding>> {
        bail!(
            "embedding provider is not configured; set {PROVIDER_ENV}=local-hash to use the preview local provider"
        )
    }
}

#[derive(Debug, Default)]
pub struct LocalHashEmbeddingProvider;

impl EmbeddingProvider for LocalHashEmbeddingProvider {
    fn provider_name(&self) -> &'static str {
        LOCAL_HASH_PROVIDER
    }

    fn model_name(&self) -> &'static str {
        LOCAL_HASH_MODEL
    }

    fn embed(&self, inputs: &[String]) -> Result<Vec<Embedding>> {
        Ok(inputs
            .iter()
            .map(|input| Embedding {
                values: local_hash_embedding(input),
            })
            .collect())
    }
}

pub fn provider_from_env() -> Result<Box<dyn EmbeddingProvider>> {
    provider_from_name(std::env::var(PROVIDER_ENV).ok().as_deref())
}

pub fn provider_from_name(name: Option<&str>) -> Result<Box<dyn EmbeddingProvider>> {
    match name.map(str::trim).filter(|name| !name.is_empty()) {
        None | Some("none" | "disabled") => Ok(Box::new(DisabledEmbeddingProvider)),
        Some("local" | LOCAL_HASH_PROVIDER) => Ok(Box::new(LocalHashEmbeddingProvider)),
        Some(name) => bail!(
            "unsupported embedding provider '{name}'; supported providers: {}",
            SUPPORTED_PROVIDER_NAMES.join(", ")
        ),
    }
}

pub fn provider_help() -> String {
    format!(
        "set {PROVIDER_ENV}=local-hash for deterministic local preview embeddings; external providers are not implemented yet"
    )
}

pub fn embed_query(provider: &dyn EmbeddingProvider, query: &str) -> Result<Embedding> {
    let embeddings = provider.embed(&[query.to_string()])?;
    embeddings
        .into_iter()
        .next()
        .context("embedding provider returned no vectors")
}

fn local_hash_embedding(input: &str) -> Vec<f32> {
    let mut vector = vec![0.0; LOCAL_HASH_DIMENSIONS];
    let mut saw_token = false;
    for token in input
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|token| !token.is_empty())
    {
        saw_token = true;
        add_token_to_vector(&mut vector, token);
    }
    if !saw_token {
        add_token_to_vector(&mut vector, input);
    }
    normalize(&mut vector);
    vector
}

fn add_token_to_vector(vector: &mut [f32], token: &str) {
    let digest = Sha256::digest(token.to_ascii_lowercase().as_bytes());
    for pair in digest.chunks_exact(2) {
        let index = pair[0] as usize % vector.len();
        let sign = if pair[1] & 1 == 0 { 1.0 } else { -1.0 };
        vector[index] += sign;
    }
}

fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector {
            *value /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_provider_returns_stable_error() {
        let provider = provider_from_name(None).unwrap();
        let error = embed_query(provider.as_ref(), "auth service").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("set CODEINSIGHT_EMBEDDING_PROVIDER=local-hash")
        );
    }

    #[test]
    fn rejects_unknown_provider_name() {
        let error = match provider_from_name(Some("remote")) {
            Ok(_) => panic!("unknown provider should be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("unsupported embedding provider"));
        assert!(error.to_string().contains("local-hash"));
    }

    #[test]
    fn local_hash_provider_returns_stable_vectors() {
        let provider = provider_from_name(Some("local-hash")).unwrap();
        assert!(provider.is_configured());
        assert_eq!(provider.provider_name(), LOCAL_HASH_PROVIDER);
        assert_eq!(provider.model_name(), LOCAL_HASH_MODEL);
        let first = embed_query(provider.as_ref(), "auth service").unwrap();
        let second = embed_query(provider.as_ref(), "auth service").unwrap();
        assert_eq!(first, second);
        assert_eq!(first.values.len(), LOCAL_HASH_DIMENSIONS);
    }
}
