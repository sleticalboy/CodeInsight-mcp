use anyhow::{Context, Result, bail};

pub const PROVIDER_ENV: &str = "CODEINSIGHT_EMBEDDING_PROVIDER";

#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    pub values: Vec<f32>,
}

pub trait EmbeddingProvider {
    fn embed(&self, inputs: &[String]) -> Result<Vec<Embedding>>;
}

#[derive(Debug, Default)]
pub struct DisabledEmbeddingProvider;

impl EmbeddingProvider for DisabledEmbeddingProvider {
    fn embed(&self, _inputs: &[String]) -> Result<Vec<Embedding>> {
        bail!(
            "embedding provider is not configured; set {PROVIDER_ENV} after enabling a supported embedding backend"
        )
    }
}

pub fn provider_from_env() -> Result<Box<dyn EmbeddingProvider>> {
    provider_from_name(std::env::var(PROVIDER_ENV).ok().as_deref())
}

pub fn provider_from_name(name: Option<&str>) -> Result<Box<dyn EmbeddingProvider>> {
    match name.map(str::trim).filter(|name| !name.is_empty()) {
        None | Some("none" | "disabled") => Ok(Box::new(DisabledEmbeddingProvider)),
        Some(name) => bail!("unsupported embedding provider '{name}'"),
    }
}

pub fn embed_query(provider: &dyn EmbeddingProvider, query: &str) -> Result<Embedding> {
    let embeddings = provider.embed(&[query.to_string()])?;
    embeddings
        .into_iter()
        .next()
        .context("embedding provider returned no vectors")
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
                .contains("embedding provider is not configured")
        );
    }

    #[test]
    fn rejects_unknown_provider_name() {
        let error = match provider_from_name(Some("remote")) {
            Ok(_) => panic!("unknown provider should be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("unsupported embedding provider"));
    }
}
