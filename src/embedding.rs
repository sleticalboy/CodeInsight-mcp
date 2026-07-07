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
pub const OPENAI_PROVIDER: &str = "openai";
pub const OPENAI_API_KEY_ENV: &str = "CODEINSIGHT_OPENAI_API_KEY";
pub const OPENAI_BASE_URL_ENV: &str = "CODEINSIGHT_OPENAI_BASE_URL";
pub const OPENAI_MODEL_ENV: &str = "CODEINSIGHT_OPENAI_EMBEDDING_MODEL";
pub const OPENAI_TIMEOUT_SECS_ENV: &str = "CODEINSIGHT_OPENAI_TIMEOUT_SECS";
pub const BATCH_SIZE_ENV: &str = "CODEINSIGHT_EMBEDDING_BATCH_SIZE";
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_OPENAI_MODEL: &str = "text-embedding-3-small";
pub const SUPPORTED_PROVIDER_NAMES: &[&str] = &[
    LOCAL_HASH_PROVIDER,
    "local",
    OLLAMA_PROVIDER,
    OPENAI_PROVIDER,
    "disabled",
    "none",
];
const LOCAL_HASH_DIMENSIONS: usize = 64;
pub const DEFAULT_OLLAMA_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_OPENAI_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_EMBEDDING_BATCH_SIZE: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingProviderConfig {
    pub provider_name: String,
    pub model_name: String,
    pub configured: bool,
    pub source: String,
    pub ollama: Option<OllamaProviderConfig>,
    pub openai: Option<OpenAiProviderConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaProviderConfig {
    pub base_url: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiProviderConfig {
    pub base_url: String,
    pub timeout_secs: u64,
    pub api_key_configured: bool,
}

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

#[derive(Debug, Clone)]
pub struct OpenAiEmbeddingProvider {
    base_url: String,
    model: String,
    api_key: String,
    timeout: Duration,
}

impl OpenAiEmbeddingProvider {
    fn from_env() -> Result<Self> {
        let config = openai_config_from_env()?;
        if !config.api_key_configured {
            bail!("{OPENAI_API_KEY_ENV} is required for the openai embedding provider");
        }
        let api_key = std::env::var(OPENAI_API_KEY_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .with_context(|| {
                format!("{OPENAI_API_KEY_ENV} is required for the openai embedding provider")
            })?;
        Ok(Self {
            base_url: config.base_url,
            model: env_or_default(OPENAI_MODEL_ENV, DEFAULT_OPENAI_MODEL),
            api_key,
            timeout: Duration::from_secs(config.timeout_secs),
        })
    }
}

impl EmbeddingProvider for OpenAiEmbeddingProvider {
    fn provider_name(&self) -> &str {
        OPENAI_PROVIDER
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn embed(&self, inputs: &[String]) -> Result<Vec<Embedding>> {
        openai_embed(
            &self.base_url,
            &self.model,
            &self.api_key,
            inputs,
            self.timeout,
        )
    }
}

pub fn provider_from_env() -> Result<Box<dyn EmbeddingProvider>> {
    provider_from_name(std::env::var(PROVIDER_ENV).ok().as_deref())
}

pub fn provider_config_from_env() -> Result<EmbeddingProviderConfig> {
    let raw_provider = std::env::var(PROVIDER_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let source = if raw_provider.is_some() {
        "environment"
    } else {
        "default"
    };

    match raw_provider.as_deref() {
        None | Some("none" | "disabled") => Ok(EmbeddingProviderConfig {
            provider_name: "disabled".to_string(),
            model_name: "disabled".to_string(),
            configured: false,
            source: source.to_string(),
            ollama: None,
            openai: None,
        }),
        Some("local" | LOCAL_HASH_PROVIDER) => Ok(EmbeddingProviderConfig {
            provider_name: LOCAL_HASH_PROVIDER.to_string(),
            model_name: LOCAL_HASH_MODEL.to_string(),
            configured: true,
            source: source.to_string(),
            ollama: None,
            openai: None,
        }),
        Some(OLLAMA_PROVIDER) => {
            let provider = OllamaEmbeddingProvider::from_env()?;
            Ok(EmbeddingProviderConfig {
                provider_name: OLLAMA_PROVIDER.to_string(),
                model_name: provider.model.clone(),
                configured: true,
                source: source.to_string(),
                ollama: Some(OllamaProviderConfig {
                    base_url: provider.base_url,
                    timeout_secs: provider.timeout.as_secs(),
                }),
                openai: None,
            })
        }
        Some(OPENAI_PROVIDER) => {
            let config = openai_config_from_env()?;
            Ok(EmbeddingProviderConfig {
                provider_name: OPENAI_PROVIDER.to_string(),
                model_name: env_or_default(OPENAI_MODEL_ENV, DEFAULT_OPENAI_MODEL),
                configured: config.api_key_configured,
                source: source.to_string(),
                ollama: None,
                openai: Some(config),
            })
        }
        Some(name) => bail!(
            "unsupported embedding provider '{name}'; supported providers: {}",
            SUPPORTED_PROVIDER_NAMES.join(", ")
        ),
    }
}

pub fn provider_from_name(name: Option<&str>) -> Result<Box<dyn EmbeddingProvider>> {
    match name.map(str::trim).filter(|name| !name.is_empty()) {
        None | Some("none" | "disabled") => Ok(Box::new(DisabledEmbeddingProvider)),
        Some("local" | LOCAL_HASH_PROVIDER) => Ok(Box::new(LocalHashEmbeddingProvider)),
        Some(OLLAMA_PROVIDER) => Ok(Box::new(OllamaEmbeddingProvider::from_env()?)),
        Some(OPENAI_PROVIDER) => Ok(Box::new(OpenAiEmbeddingProvider::from_env()?)),
        Some(name) => bail!(
            "unsupported embedding provider '{name}'; supported providers: {}",
            SUPPORTED_PROVIDER_NAMES.join(", ")
        ),
    }
}

pub fn provider_help() -> String {
    format!(
        "set {PROVIDER_ENV}=local-hash for deterministic local preview embeddings, {PROVIDER_ENV}=ollama for local Ollama embeddings, or {PROVIDER_ENV}=openai for OpenAI-compatible embeddings"
    )
}

pub fn batch_size_from_env() -> Result<usize> {
    match std::env::var(BATCH_SIZE_ENV)
        .ok()
        .map(|value| value.trim().to_string())
    {
        Some(value) if !value.is_empty() => {
            let parsed = value
                .parse::<usize>()
                .with_context(|| format!("{BATCH_SIZE_ENV} must be a positive integer"))?;
            Ok(parsed.max(1))
        }
        _ => Ok(DEFAULT_EMBEDDING_BATCH_SIZE),
    }
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

#[derive(Debug, Deserialize)]
struct OpenAiEmbedResponse {
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
    index: usize,
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

fn openai_config_from_env() -> Result<OpenAiProviderConfig> {
    let base_url = normalize_http_or_https_base_url(&env_or_default(
        OPENAI_BASE_URL_ENV,
        DEFAULT_OPENAI_BASE_URL,
    ))?;
    let model = env_or_default(OPENAI_MODEL_ENV, DEFAULT_OPENAI_MODEL);
    if model.trim().is_empty() {
        bail!("{OPENAI_MODEL_ENV} must not be empty for the openai embedding provider");
    }
    let timeout_secs = env_u64_or_default(OPENAI_TIMEOUT_SECS_ENV, DEFAULT_OPENAI_TIMEOUT_SECS)?;
    let api_key_configured = std::env::var(OPENAI_API_KEY_ENV)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);

    Ok(OpenAiProviderConfig {
        base_url,
        timeout_secs,
        api_key_configured,
    })
}

fn normalize_http_or_https_base_url(base_url: &str) -> Result<String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    let rest = if let Some(rest) = trimmed.strip_prefix("https://") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        rest
    } else {
        bail!(
            "{OPENAI_BASE_URL_ENV} must start with http:// or https:// for the openai embedding provider"
        )
    };
    let authority = rest.split('/').next().unwrap_or_default();
    if authority.is_empty() {
        bail!("{OPENAI_BASE_URL_ENV} must include a host");
    }
    Ok(trimmed.to_string())
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
    if response.embeddings.len() != inputs.len() {
        bail!(
            "ollama embedding provider returned {} vectors for {} inputs",
            response.embeddings.len(),
            inputs.len()
        );
    }
    Ok(response
        .embeddings
        .into_iter()
        .map(|values| Embedding { values })
        .collect())
}

fn openai_embed(
    base_url: &str,
    model: &str,
    api_key: &str,
    inputs: &[String],
    timeout: Duration,
) -> Result<Vec<Embedding>> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let api_key = api_key.trim();
    if api_key.is_empty() {
        bail!("{OPENAI_API_KEY_ENV} is required for the openai embedding provider");
    }
    let endpoint = openai_embeddings_endpoint(base_url)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .context("failed to build openai embedding HTTP client")?;
    let response = client
        .post(&endpoint)
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": model,
            "input": inputs,
            "encoding_format": "float",
        }))
        .send()
        .with_context(|| {
            format!(
                "openai embedding provider is unreachable at {endpoint}; set {OPENAI_BASE_URL_ENV}"
            )
        })?;
    let status = response.status();
    let body = response
        .text()
        .context("failed to read openai embedding response")?;
    if !status.is_success() {
        bail!("openai embedding provider returned HTTP {status}: {body}");
    }
    let response = serde_json::from_str::<OpenAiEmbedResponse>(&body)
        .context("openai embedding provider returned invalid JSON")?;
    openai_embeddings_from_response(response, inputs.len())
}

fn openai_embeddings_endpoint(base_url: &str) -> Result<String> {
    let base_url = normalize_http_or_https_base_url(base_url)?;
    Ok(format!("{base_url}/embeddings"))
}

fn openai_embeddings_from_response(
    response: OpenAiEmbedResponse,
    input_count: usize,
) -> Result<Vec<Embedding>> {
    if response.data.len() != input_count {
        bail!(
            "openai embedding provider returned {} vectors for {} inputs",
            response.data.len(),
            input_count
        );
    }
    let mut embeddings = vec![None; input_count];
    for item in response.data {
        if item.index >= input_count {
            bail!(
                "openai embedding provider returned index {} for {} inputs",
                item.index,
                input_count
            );
        }
        if embeddings[item.index].is_some() {
            bail!(
                "openai embedding provider returned duplicate index {}",
                item.index
            );
        }
        embeddings[item.index] = Some(Embedding {
            values: item.embedding,
        });
    }

    embeddings
        .into_iter()
        .enumerate()
        .map(|(index, embedding)| {
            embedding.with_context(|| {
                format!("openai embedding provider omitted index {index} from response")
            })
        })
        .collect()
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

    let host_header = if base_url.port == 80 {
        base_url.host.clone()
    } else {
        format!("{}:{}", base_url.host, base_url.port)
    };
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n{body}",
        host = host_header,
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
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::Mutex,
        thread::{self, JoinHandle},
    };

    use super::*;
    use serde_json::Value;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[derive(Debug)]
    struct MockRequest {
        request_line: String,
        headers: String,
        body: String,
    }

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
    fn provider_config_reports_default_disabled_provider() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var(PROVIDER_ENV).ok();
        unsafe {
            std::env::remove_var(PROVIDER_ENV);
        }

        let config = provider_config_from_env().unwrap();

        assert_eq!(config.provider_name, "disabled");
        assert_eq!(config.model_name, "disabled");
        assert!(!config.configured);
        assert_eq!(config.source, "default");
        assert_eq!(config.ollama, None);
        assert_eq!(config.openai, None);
        restore_env(PROVIDER_ENV, previous);
    }

    #[test]
    fn batch_size_from_env_reports_default_and_normalizes_zero() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var(BATCH_SIZE_ENV).ok();
        unsafe {
            std::env::remove_var(BATCH_SIZE_ENV);
        }
        assert_eq!(batch_size_from_env().unwrap(), DEFAULT_EMBEDDING_BATCH_SIZE);

        unsafe {
            std::env::set_var(BATCH_SIZE_ENV, "3");
        }
        assert_eq!(batch_size_from_env().unwrap(), 3);

        unsafe {
            std::env::set_var(BATCH_SIZE_ENV, "0");
        }
        assert_eq!(batch_size_from_env().unwrap(), 1);

        restore_env(BATCH_SIZE_ENV, previous);
    }

    #[test]
    fn batch_size_from_env_rejects_non_integer() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var(BATCH_SIZE_ENV).ok();
        unsafe {
            std::env::set_var(BATCH_SIZE_ENV, "large");
        }

        let error = batch_size_from_env().unwrap_err();

        assert!(
            error
                .to_string()
                .contains("CODEINSIGHT_EMBEDDING_BATCH_SIZE must be a positive integer")
        );
        restore_env(BATCH_SIZE_ENV, previous);
    }

    #[test]
    fn provider_config_reports_ollama_settings() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous_provider = std::env::var(PROVIDER_ENV).ok();
        let previous_base_url = std::env::var(OLLAMA_BASE_URL_ENV).ok();
        let previous_model = std::env::var(OLLAMA_MODEL_ENV).ok();
        let previous_timeout = std::env::var(OLLAMA_TIMEOUT_SECS_ENV).ok();
        unsafe {
            std::env::set_var(PROVIDER_ENV, OLLAMA_PROVIDER);
            std::env::set_var(OLLAMA_BASE_URL_ENV, "http://127.0.0.1:9999/custom");
            std::env::set_var(OLLAMA_MODEL_ENV, "nomic-embed-text");
            std::env::set_var(OLLAMA_TIMEOUT_SECS_ENV, "5");
        }

        let config = provider_config_from_env().unwrap();

        assert_eq!(config.provider_name, OLLAMA_PROVIDER);
        assert_eq!(config.model_name, "nomic-embed-text");
        assert!(config.configured);
        assert_eq!(config.source, "environment");
        assert_eq!(
            config.ollama,
            Some(OllamaProviderConfig {
                base_url: "http://127.0.0.1:9999/custom".to_string(),
                timeout_secs: 5,
            })
        );
        assert_eq!(config.openai, None);
        restore_env(PROVIDER_ENV, previous_provider);
        restore_env(OLLAMA_BASE_URL_ENV, previous_base_url);
        restore_env(OLLAMA_MODEL_ENV, previous_model);
        restore_env(OLLAMA_TIMEOUT_SECS_ENV, previous_timeout);
    }

    #[test]
    fn provider_config_reports_openai_settings_without_exposing_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous_provider = std::env::var(PROVIDER_ENV).ok();
        let previous_api_key = std::env::var(OPENAI_API_KEY_ENV).ok();
        let previous_base_url = std::env::var(OPENAI_BASE_URL_ENV).ok();
        let previous_model = std::env::var(OPENAI_MODEL_ENV).ok();
        let previous_timeout = std::env::var(OPENAI_TIMEOUT_SECS_ENV).ok();
        unsafe {
            std::env::set_var(PROVIDER_ENV, OPENAI_PROVIDER);
            std::env::set_var(OPENAI_API_KEY_ENV, "sk-test-secret");
            std::env::set_var(OPENAI_BASE_URL_ENV, "https://example.test/v1/");
            std::env::set_var(OPENAI_MODEL_ENV, "text-embedding-3-large");
            std::env::set_var(OPENAI_TIMEOUT_SECS_ENV, "9");
        }

        let config = provider_config_from_env().unwrap();

        assert_eq!(config.provider_name, OPENAI_PROVIDER);
        assert_eq!(config.model_name, "text-embedding-3-large");
        assert!(config.configured);
        assert_eq!(config.ollama, None);
        assert_eq!(
            config.openai,
            Some(OpenAiProviderConfig {
                base_url: "https://example.test/v1".to_string(),
                timeout_secs: 9,
                api_key_configured: true,
            })
        );
        restore_env(PROVIDER_ENV, previous_provider);
        restore_env(OPENAI_API_KEY_ENV, previous_api_key);
        restore_env(OPENAI_BASE_URL_ENV, previous_base_url);
        restore_env(OPENAI_MODEL_ENV, previous_model);
        restore_env(OPENAI_TIMEOUT_SECS_ENV, previous_timeout);
    }

    #[test]
    fn provider_config_reports_missing_openai_api_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous_provider = std::env::var(PROVIDER_ENV).ok();
        let previous_api_key = std::env::var(OPENAI_API_KEY_ENV).ok();
        unsafe {
            std::env::set_var(PROVIDER_ENV, OPENAI_PROVIDER);
            std::env::remove_var(OPENAI_API_KEY_ENV);
        }

        let config = provider_config_from_env().unwrap();

        assert_eq!(config.provider_name, OPENAI_PROVIDER);
        assert_eq!(config.model_name, DEFAULT_OPENAI_MODEL);
        assert!(!config.configured);
        assert_eq!(
            config.openai,
            Some(OpenAiProviderConfig {
                base_url: DEFAULT_OPENAI_BASE_URL.to_string(),
                timeout_secs: DEFAULT_OPENAI_TIMEOUT_SECS,
                api_key_configured: false,
            })
        );
        restore_env(PROVIDER_ENV, previous_provider);
        restore_env(OPENAI_API_KEY_ENV, previous_api_key);
    }

    #[test]
    fn openai_provider_requires_api_key_for_embedding_calls() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous_api_key = std::env::var(OPENAI_API_KEY_ENV).ok();
        unsafe {
            std::env::remove_var(OPENAI_API_KEY_ENV);
        }

        let error = match provider_from_name(Some(OPENAI_PROVIDER)) {
            Ok(_) => panic!("openai provider should require an API key for embedding calls"),
            Err(error) => error,
        };

        assert!(error.to_string().contains(OPENAI_API_KEY_ENV));
        restore_env(OPENAI_API_KEY_ENV, previous_api_key);
    }

    #[test]
    fn rejects_invalid_openai_base_url_scheme() {
        let error = normalize_http_or_https_base_url("ftp://example.test/v1").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("must start with http:// or https://")
        );
    }

    #[test]
    fn openai_provider_posts_expected_embeddings_request_body() {
        let response = json_response(
            r#"{"data":[{"embedding":[0.0,1.0],"index":1},{"embedding":[1.0,0.0],"index":0}]}"#,
        );
        let (base_url, handle) = serve_one_ollama_request(response);
        let embeddings = openai_embed(
            &format!("{base_url}/v1"),
            "text-embedding-test",
            "sk-test-secret",
            &["alpha".to_string(), "beta".to_string()],
            Duration::from_secs(2),
        )
        .unwrap();

        assert_eq!(
            embeddings,
            vec![
                Embedding {
                    values: vec![1.0, 0.0]
                },
                Embedding {
                    values: vec![0.0, 1.0]
                }
            ]
        );
        let request = handle.join().unwrap();
        assert_eq!(request.request_line, "POST /v1/embeddings HTTP/1.1");
        assert!(
            request
                .headers
                .contains("authorization: Bearer sk-test-secret")
        );
        assert!(request.headers.contains("content-type: application/json"));
        let body = serde_json::from_str::<Value>(&request.body).unwrap();
        assert_eq!(body["model"], "text-embedding-test");
        assert_eq!(body["input"], serde_json::json!(["alpha", "beta"]));
        assert_eq!(body["encoding_format"], "float");
    }

    #[test]
    fn openai_provider_reports_non_200_response_without_api_key() {
        let response =
            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 7\r\nConnection: close\r\n\r\ndenied!"
                .to_string();
        let (base_url, handle) = serve_one_ollama_request(response);

        let error = openai_embed(
            &base_url,
            "text-embedding-test",
            "sk-test-secret",
            &["alpha".to_string()],
            Duration::from_secs(2),
        )
        .unwrap_err();

        assert!(error.to_string().contains("401 Unauthorized"));
        assert!(!error.to_string().contains("sk-test-secret"));
        let _request = handle.join().unwrap();
    }

    #[test]
    fn openai_provider_rejects_embedding_count_mismatch() {
        let response = json_response(r#"{"data":[{"embedding":[1.0],"index":0}]}"#);
        let (base_url, handle) = serve_one_ollama_request(response);

        let error = openai_embed(
            &base_url,
            "text-embedding-test",
            "sk-test-secret",
            &["alpha".to_string(), "beta".to_string()],
            Duration::from_secs(2),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("returned 1 vectors for 2 inputs")
        );
        let _request = handle.join().unwrap();
    }

    #[test]
    fn openai_provider_rejects_duplicate_embedding_index() {
        let response = json_response(
            r#"{"data":[{"embedding":[1.0],"index":0},{"embedding":[2.0],"index":0}]}"#,
        );
        let (base_url, handle) = serve_one_ollama_request(response);

        let error = openai_embed(
            &base_url,
            "text-embedding-test",
            "sk-test-secret",
            &["alpha".to_string(), "beta".to_string()],
            Duration::from_secs(2),
        )
        .unwrap_err();

        assert!(error.to_string().contains("duplicate index 0"));
        let _request = handle.join().unwrap();
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

    #[test]
    fn ollama_provider_posts_expected_embed_request_body() {
        let response = json_response(r#"{"embeddings":[[1.0,0.0],[0.0,1.0]]}"#);
        let (base_url, handle) = serve_one_ollama_request(response);

        let embeddings = ollama_embed(
            &base_url,
            "unit-embed",
            &["alpha".to_string(), "beta".to_string()],
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(
            embeddings,
            vec![
                Embedding {
                    values: vec![1.0, 0.0]
                },
                Embedding {
                    values: vec![0.0, 1.0]
                }
            ]
        );

        let request = handle.join().unwrap();
        assert_eq!(request.request_line, "POST /api/embed HTTP/1.1");
        assert!(request.headers.contains("Content-Type: application/json"));
        assert!(request.headers.contains("Accept: application/json"));
        let body = serde_json::from_str::<Value>(&request.body).unwrap();
        assert_eq!(body["model"], "unit-embed");
        assert_eq!(body["input"], serde_json::json!(["alpha", "beta"]));
    }

    #[test]
    fn ollama_provider_accepts_chunked_embed_response() {
        let body = r#"{"embeddings":[[0.25],[0.75]]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{}",
            chunked_body(body)
        );
        let (base_url, handle) = serve_one_ollama_request(response);

        let embeddings = ollama_embed(
            &base_url,
            "unit-embed",
            &["first".to_string(), "second".to_string()],
            Duration::from_secs(2),
        )
        .unwrap();

        assert_eq!(embeddings[0].values, vec![0.25]);
        assert_eq!(embeddings[1].values, vec![0.75]);
        let request = handle.join().unwrap();
        assert_eq!(request.request_line, "POST /api/embed HTTP/1.1");
    }

    #[test]
    fn ollama_provider_reports_non_200_response() {
        let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 4\r\nConnection: close\r\n\r\nboom".to_string();
        let (base_url, handle) = serve_one_ollama_request(response);

        let error = ollama_embed(
            &base_url,
            "unit-embed",
            &["alpha".to_string()],
            Duration::from_secs(2),
        )
        .unwrap_err();

        assert!(error.to_string().contains("500 Internal Server Error"));
        let _request = handle.join().unwrap();
    }

    #[test]
    fn ollama_provider_rejects_embedding_count_mismatch() {
        let response = json_response(r#"{"embeddings":[[1.0]]}"#);
        let (base_url, handle) = serve_one_ollama_request(response);

        let error = ollama_embed(
            &base_url,
            "unit-embed",
            &["alpha".to_string(), "beta".to_string()],
            Duration::from_secs(2),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("returned 1 vectors for 2 inputs")
        );
        let _request = handle.join().unwrap();
    }

    fn serve_one_ollama_request(response: String) -> (String, JoinHandle<MockRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 512];
            let header_end = loop {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0, "client closed connection before headers");
                request.extend_from_slice(&buffer[..read]);
                if let Some(header_end) = find_header_end(&request) {
                    break header_end;
                }
            };

            let headers = String::from_utf8(request[..header_end].to_vec()).unwrap();
            let content_length = content_length(&headers);
            let body_start = header_end + 4;
            while request.len() < body_start + content_length {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0, "client closed connection before body");
                request.extend_from_slice(&buffer[..read]);
            }

            stream.write_all(response.as_bytes()).unwrap();
            let body = String::from_utf8(request[body_start..body_start + content_length].to_vec())
                .unwrap();
            MockRequest {
                request_line: headers.lines().next().unwrap_or_default().to_string(),
                headers,
                body,
            }
        });

        (format!("http://{address}"), handle)
    }

    fn find_header_end(bytes: &[u8]) -> Option<usize> {
        bytes.windows(4).position(|window| window == b"\r\n\r\n")
    }

    fn content_length(headers: &str) -> usize {
        headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    Some(value.trim().parse::<usize>().unwrap())
                } else {
                    None
                }
            })
            .unwrap_or(0)
    }

    fn json_response(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }

    fn chunked_body(body: &str) -> String {
        format!("{:x}\r\n{}\r\n0\r\n\r\n", body.len(), body)
    }

    fn restore_env(key: &str, value: Option<String>) {
        unsafe {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
