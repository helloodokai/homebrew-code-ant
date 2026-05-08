use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use reqwest::Client;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderParams {
    pub host: String,
    pub api_key: Option<String>,
    pub model: String,
}

#[derive(Debug, Clone)]
pub enum ModelProvider {
    Ollama(ProviderParams),
    OpenAi(ProviderParams),
    Anthropic(ProviderParams),
    Mock { response: String },
}

impl ModelProvider {
    pub async fn generate(&self, system: &str, prompt: &str) -> Result<String> {
        match self {
            ModelProvider::Ollama(params) => ollama_generate(params, system, prompt).await,
            ModelProvider::OpenAi(params) => openai_generate(params, system, prompt).await,
            ModelProvider::Anthropic(params) => anthropic_generate(params, system, prompt).await,
            ModelProvider::Mock { response } => Ok(response.clone()),
        }
    }
}

pub fn build_provider(name: &str, params: ProviderParams) -> Result<ModelProvider> {
    match name {
        "ollama_cloud" | "ollama_local" => Ok(ModelProvider::Ollama(params)),
        "openai" => Ok(ModelProvider::OpenAi(params)),
        "anthropic" => Ok(ModelProvider::Anthropic(params)),
        "mock" => Ok(ModelProvider::Mock { response: params.model }),
        _ => bail!("Unknown provider: {}", name),
    }
}

async fn ollama_generate(params: &ProviderParams, system: &str, prompt: &str) -> Result<String> {
    let client = Client::new();
    let url = format!("{}/api/generate", params.host.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": params.model,
        "system": system,
        "prompt": prompt,
        "stream": false,
        "options": {
            "temperature": 0.1,
        },
    });
    let mut req = client.post(&url).json(&body);
    if let Some(key) = &params.api_key {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        let text = resp.text().await?;
        bail!("Ollama API error: {}", text);
    }
    let json: serde_json::Value = resp.json().await?;
    let text = json
        .get("response")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(text)
}

async fn openai_generate(params: &ProviderParams, system: &str, prompt: &str) -> Result<String> {
    let client = Client::new();
    let url = "https://api.openai.com/v1/chat/completions";
    let body = serde_json::json!({
        "model": params.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt},
        ],
        "temperature": 0.1,
    });
    let mut req = client.post(url).json(&body);
    if let Some(key) = &params.api_key {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        let text = resp.text().await?;
        bail!("OpenAI API error: {}", text);
    }
    let json: serde_json::Value = resp.json().await?;
    let text = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(text)
}

async fn anthropic_generate(params: &ProviderParams, system: &str, prompt: &str) -> Result<String> {
    let client = Client::new();
    let url = "https://api.anthropic.com/v1/messages";
    let body = serde_json::json!({
        "model": params.model,
        "max_tokens": 4096,
        "system": system,
        "messages": [
            {"role": "user", "content": prompt},
        ],
        "temperature": 0.1,
    });
    let mut req = client
        .post(url)
        .json(&body)
        .header("anthropic-version", "2023-06-01");
    if let Some(key) = &params.api_key {
        req = req.header("x-api-key", key);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        let text = resp.text().await?;
        bail!("Anthropic API error: {}", text);
    }
    let json: serde_json::Value = resp.json().await?;
    let text = json
        .get("content")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_provider_ollama() {
        let params = ProviderParams {
            host: "http://localhost:11434".to_string(),
            api_key: None,
            model: "qwen2.5-coder:7b".to_string(),
        };
        let p = build_provider("ollama_local", params).unwrap();
        matches!(p, ModelProvider::Ollama(_));
    }

    #[test]
    fn test_build_provider_openai() {
        let params = ProviderParams {
            host: "https://api.openai.com".to_string(),
            api_key: Some("sk-test".to_string()),
            model: "gpt-4".to_string(),
        };
        let p = build_provider("openai", params).unwrap();
        matches!(p, ModelProvider::OpenAi(_));
    }

    #[test]
    fn test_build_provider_anthropic() {
        let params = ProviderParams {
            host: "https://api.anthropic.com".to_string(),
            api_key: Some("sk-ant-test".to_string()),
            model: "claude-sonnet-4-6".to_string(),
        };
        let p = build_provider("anthropic", params).unwrap();
        matches!(p, ModelProvider::Anthropic(_));
    }

    #[test]
    fn test_build_provider_unknown() {
        let params = ProviderParams {
            host: "http://x".to_string(),
            api_key: None,
            model: "x".to_string(),
        };
        assert!(build_provider("unknown", params).is_err());
    }
}
