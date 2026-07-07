use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const PROVIDER_ENV: &str = "CODEINSIGHT_EMBEDDING_PROVIDER";
pub const LOCAL_HASH_PROVIDER: &str = "local-hash";
pub const LOCAL_HASH_MODEL: &str = "local-hash-v1";
pub const OLLAMA_PROVIDER: &str = "ollama";
pub const OLLAMA_BASE_URL_ENV: &str = "CODEINSIGHT_OLLAMA_BASE_URL";
pub const OLLAMA_MODEL_ENV: &str = "CODEINSIGHT_OLLAMA_EMBEDDING_MODEL";
pub const OLLAMA_TIMEOUT_SECS_ENV: &str = "CODEINSIGHT_OLLAMA_TIMEOUT_SECS";
pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
pub const DEFAULT_OLLAMA_MODEL: &str = "embeddinggemma";
pub const SUPPORTED_PROVIDER_NAMES: &[&str] = &[
    LOCAL_HASH_PROVIDER,
    "local",
    OLLAMA_PROVIDER,
    "disabled",
    "none",
];
const LOCAL_HASH_DIMENSIONS: usize = 64;
const DEFAULT_OLLAMA_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    pub values: Vec<f32>,
}

pub trait EmbeddingProvider {
    fn provider_name(&self) -> &str;
    fn model_name(&self) -> &str;
    fn is_configured(&self) -> bool {
        true
    }
    fn embed(&self, inputs: &[String]) -> Result<Vec<Embedding>>;
}

#[derive(Debug, Default)]
pub struct DisabledEmbeddingProvider;

impl EmbeddingProvider for DisabledEmbeddingProvider {
    fn provider_name(&self) -> &str {
        "disabled"
    }

    fn model_name(&self) -> &str {
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
    fn provider_name(&self) -> &str {
        LOCAL_HASH_PROVIDER
    }

    fn model_name(&self) -> &str {
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

#[derive(Debug, Clone)]
pub struct OllamaEmbeddingProvider {
    base_url: String,
    model: String,
    timeout: Duration,
}

impl OllamaEmbeddingProvider {
    fn from_env() -> Result<Self> {
        let base_url = env_or_default(OLLAMA_BASE_URL_ENV, DEFAULT_OLLAMA_BASE_URL);
        let model = env_or_default(OLLAMA_MODEL_ENV, DEFAULT_OLLAMA_MODEL);
        let timeout = Duration::from_secs(env_u64_or_default(
            OLLAMA_TIMEOUT_SECS_ENV,
            DEFAULT_OLLAMA_TIMEOUT_SECS,
        )?);
        parse_http_base_url(&base_url)?;
        if model.trim().is_empty() {
            bail!("{OLLAMA_MODEL_ENV} must not be empty for the ollama embedding provider");
        }
        Ok(Self {
            base_url,
            model,
            timeout,
        })
    }
}

impl EmbeddingProvider for OllamaEmbeddingProvider {
    fn provider_name(&self) -> &str {
        OLLAMA_PROVIDER
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn embed(&self, inputs: &[String]) -> Result<Vec<Embedding>> {
        ollama_embed(&self.base_url, &self.model, inputs, self.timeout)
    }
}

pub fn provider_from_env() -> Result<Box<dyn EmbeddingProvider>> {
    provider_from_name(std::env::var(PROVIDER_ENV).ok().as_deref())
}

pub fn provider_from_name(name: Option<&str>) -> Result<Box<dyn EmbeddingProvider>> {
    match name.map(str::trim).filter(|name| !name.is_empty()) {
        None | Some("none" | "disabled") => Ok(Box::new(DisabledEmbeddingProvider)),
        Some("local" | LOCAL_HASH_PROVIDER) => Ok(Box::new(LocalHashEmbeddingProvider)),
        Some(OLLAMA_PROVIDER) => Ok(Box::new(OllamaEmbeddingProvider::from_env()?)),
        Some(name) => bail!(
            "unsupported embedding provider '{name}'; supported providers: {}",
            SUPPORTED_PROVIDER_NAMES.join(", ")
        ),
    }
}

pub fn provider_help() -> String {
    format!(
        "set {PROVIDER_ENV}=local-hash for deterministic local preview embeddings or {PROVIDER_ENV}=ollama for local Ollama embeddings"
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

#[derive(Debug)]
struct HttpBaseUrl {
    host: String,
    port: u16,
    path_prefix: String,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

fn env_or_default(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_u64_or_default(key: &str, default: u64) -> Result<u64> {
    match std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
    {
        Some(value) if !value.is_empty() => {
            let parsed = value
                .parse::<u64>()
                .with_context(|| format!("{key} must be a positive integer"))?;
            Ok(parsed.max(1))
        }
        _ => Ok(default),
    }
}

fn parse_http_base_url(base_url: &str) -> Result<HttpBaseUrl> {
    let trimmed = base_url.trim().trim_end_matches('/');
    let rest = if let Some(rest) = trimmed.strip_prefix("http://") {
        rest
    } else if trimmed.starts_with("https://") {
        bail!(
            "ollama provider currently supports http base URLs only; set {OLLAMA_BASE_URL_ENV}=http://127.0.0.1:11434"
        )
    } else {
        bail!("{OLLAMA_BASE_URL_ENV} must start with http:// for the ollama embedding provider")
    };

    let (authority, path_prefix) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty() {
        bail!("{OLLAMA_BASE_URL_ENV} must include a host");
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => {
            let parsed_port = port
                .parse::<u16>()
                .with_context(|| format!("{OLLAMA_BASE_URL_ENV} has an invalid port"))?;
            (host.to_string(), parsed_port)
        }
        _ => (authority.to_string(), 80),
    };

    Ok(HttpBaseUrl {
        host,
        port,
        path_prefix: path_prefix.trim_matches('/').to_string(),
    })
}

fn ollama_embed(
    base_url: &str,
    model: &str,
    inputs: &[String],
    timeout: Duration,
) -> Result<Vec<Embedding>> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }

    let parsed = parse_http_base_url(base_url)?;
    let path = if parsed.path_prefix.is_empty() {
        "/api/embed".to_string()
    } else {
        format!("/{}/api/embed", parsed.path_prefix)
    };
    let body = serde_json::json!({
        "model": model,
        "input": inputs,
    })
    .to_string();

    let response_body = http_post_json(&parsed, &path, &body, timeout)?;
    let response = serde_json::from_str::<OllamaEmbedResponse>(&response_body)
        .context("ollama embedding provider returned invalid JSON")?;
    Ok(response
        .embeddings
        .into_iter()
        .map(|values| Embedding { values })
        .collect())
}

fn http_post_json(
    base_url: &HttpBaseUrl,
    path: &str,
    body: &str,
    timeout: Duration,
) -> Result<String> {
    let mut addresses = (base_url.host.as_str(), base_url.port)
        .to_socket_addrs()
        .with_context(|| format!("failed to resolve ollama host '{}'", base_url.host))?;
    let address = addresses
        .next()
        .with_context(|| format!("failed to resolve ollama host '{}'", base_url.host))?;
    let mut stream = TcpStream::connect_timeout(&address, timeout).with_context(|| {
        format!(
            "ollama embedding provider is unreachable at http://{}:{}; start Ollama or set {OLLAMA_BASE_URL_ENV}",
            base_url.host, base_url.port
        )
    })?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n{body}",
        host = base_url.host,
        length = body.len(),
    );
    stream
        .write_all(request.as_bytes())
        .context("failed to write ollama embedding request")?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .context("failed to read ollama embedding response")?;
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .context("ollama embedding provider returned an invalid HTTP response")?;
    let status_line = headers.lines().next().unwrap_or_default();
    if !status_line.contains(" 200 ") {
        bail!("ollama embedding provider returned {status_line}: {body}");
    }
    if headers.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with("transfer-encoding: chunked")
    }) {
        return decode_chunked_body(body);
    }
    Ok(body.to_string())
}

fn decode_chunked_body(body: &str) -> Result<String> {
    let mut rest = body;
    let mut decoded = String::new();
    loop {
        let (size_line, after_size) = rest
            .split_once("\r\n")
            .context("ollama embedding provider returned an invalid chunked response")?;
        let size_hex = size_line.split(';').next().unwrap_or(size_line).trim();
        let size = usize::from_str_radix(size_hex, 16)
            .context("ollama embedding provider returned an invalid chunk size")?;
        if size == 0 {
            break;
        }
        if after_size.len() < size + 2 {
            bail!("ollama embedding provider returned a truncated chunked response");
        }
        let (chunk, after_chunk) = after_size.split_at(size);
        decoded.push_str(chunk);
        rest = after_chunk
            .strip_prefix("\r\n")
            .context("ollama embedding provider returned an invalid chunk terminator")?;
    }
    Ok(decoded)
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

    #[test]
    fn parses_ollama_http_base_url() {
        let parsed = parse_http_base_url("http://127.0.0.1:11434").unwrap();
        assert_eq!(parsed.host, "127.0.0.1");
        assert_eq!(parsed.port, 11434);
        assert_eq!(parsed.path_prefix, "");

        let parsed = parse_http_base_url("http://localhost:11434/custom/").unwrap();
        assert_eq!(parsed.host, "localhost");
        assert_eq!(parsed.port, 11434);
        assert_eq!(parsed.path_prefix, "custom");
    }

    #[test]
    fn rejects_unsupported_ollama_base_url_scheme() {
        let error = parse_http_base_url("https://localhost:11434").unwrap_err();
        assert!(error.to_string().contains("supports http base URLs only"));
    }

    #[test]
    fn ollama_provider_reports_dynamic_model_name() {
        let provider = OllamaEmbeddingProvider {
            base_url: DEFAULT_OLLAMA_BASE_URL.to_string(),
            model: "nomic-embed-text".to_string(),
            timeout: Duration::from_secs(1),
        };
        assert_eq!(provider.provider_name(), OLLAMA_PROVIDER);
        assert_eq!(provider.model_name(), "nomic-embed-text");
    }

    #[test]
    fn decodes_chunked_ollama_response_body() {
        let decoded = decode_chunked_body("7\r\n{\"a\":1}\r\n0\r\n\r\n").unwrap();
        assert_eq!(decoded, "{\"a\":1}");
    }
}
