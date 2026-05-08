use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub provider: ProviderConfigSection,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfigSection {
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default, flatten)]
    pub entries: HashMap<String, ProviderEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub host: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

/// Load config by merging user-level and project-level files.
/// Project-level overrides user-level; CLI args override both.
pub fn load_config() -> Result<Config> {
    let mut cfg = Config::default();

    // 1. User-level config: ~/.code-ant/config.toml
    if let Some(user_config) = user_config_path() {
        if let Ok(content) = std::fs::read_to_string(&user_config) {
            if let Ok(parsed) = toml::from_str::<Config>(&content) {
                merge_config(&mut cfg, parsed);
            }
        }
    }

    // 2. Project-level config: ./.code-ant/config.toml
    if let Ok(content) = std::fs::read_to_string(".code-ant/config.toml") {
        if let Ok(parsed) = toml::from_str::<Config>(&content) {
            merge_config(&mut cfg, parsed);
        }
    }

    // 3. Legacy .charter.toml support (read provider keys under [models])
    if let Ok(content) = std::fs::read_to_string(".charter.toml") {
        if let Ok(doc) = toml::from_str::<toml::Value>(&content) {
            if let Some(models_table) = doc.get("models").and_then(|m| m.as_table()) {
                for (key, value) in models_table.iter() {
                    if key == "profiles" || key == "default_profile" {
                        continue;
                    }
                    let host = value.get("host").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let api_key = value.get("api_key").and_then(|v| v.as_str()).map(|s| s.to_string());
                    if host.is_some() || api_key.is_some() {
                        let entry = ProviderEntry { host, api_key, model: None };
                        cfg.provider.entries.insert(key.to_string(), entry);
                    }
                }
            }
        }
    }

    Ok(cfg)
}

fn user_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".code-ant").join("config.toml"))
}

fn merge_config(base: &mut Config, overlay: Config) {
    if let Some(def) = overlay.provider.default {
        base.provider.default = Some(def);
    }
    for (key, entry) in overlay.provider.entries {
        let base_entry = base.provider.entries.entry(key).or_default();
        if entry.host.is_some() {
            base_entry.host = entry.host;
        }
        if entry.api_key.is_some() {
            base_entry.api_key = entry.api_key;
        }
        if entry.model.is_some() {
            base_entry.model = entry.model;
        }
    }
}

/// Resolve the effective provider configuration given CLI overrides.
pub fn resolve_provider_config(
    cfg: &Config,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
) -> (String, String, Option<String>, Option<String>) {
    let provider_name = cli_provider
        .map(|s| s.to_string())
        .or_else(|| std::env::var("CODE_ANT_PROVIDER").ok())
        .or_else(|| cfg.provider.default.clone())
        .unwrap_or_else(|| "ollama_cloud".to_string());

    let entry = cfg.provider.entries.get(&provider_name);

    let model = cli_model
        .map(|s| s.to_string())
        .or_else(|| std::env::var("CODE_ANT_MODEL").ok())
        .or_else(|| entry.and_then(|e| e.model.clone()))
        .unwrap_or_else(|| default_model(&provider_name).to_string());

    let host = entry
        .and_then(|e| e.host.clone())
        .unwrap_or_else(|| default_host(&provider_name).to_string());

    let api_key = cli_api_key
        .map(|s| s.to_string())
        .or_else(|| std::env::var("CODE_ANT_API_KEY").ok())
        .or_else(|| entry.and_then(|e| e.api_key.clone()));

    (provider_name, model, Some(host), api_key)
}

fn default_host(provider: &str) -> &'static str {
    match provider {
        "ollama_cloud" => "https://api.ollama.com",
        "ollama_local" => "http://localhost:11434",
        "openai" => "https://api.openai.com",
        "anthropic" => "https://api.anthropic.com",
        _ => "",
    }
}

fn default_model(provider: &str) -> &'static str {
    match provider {
        "ollama_cloud" | "ollama_local" => "qwen2.5-coder:7b",
        "openai" => "gpt-4",
        "anthropic" => "claude-sonnet-4-6",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_config() {
        let mut base = Config::default();
        base.provider.default = Some("ollama_cloud".to_string());
        base.provider.entries.insert(
            "ollama_cloud".to_string(),
            ProviderEntry {
                host: Some("http://old".to_string()),
                api_key: Some("old_key".to_string()),
                model: Some("old_model".to_string()),
            },
        );

        let mut overlay = Config::default();
        overlay.provider.default = Some("openai".to_string());
        overlay.provider.entries.insert(
            "ollama_cloud".to_string(),
            ProviderEntry {
                host: Some("http://new".to_string()),
                api_key: None,
                model: None,
            },
        );

        merge_config(&mut base, overlay);
        assert_eq!(base.provider.default, Some("openai".to_string()));
        let entry = base.provider.entries.get("ollama_cloud").unwrap();
        assert_eq!(entry.host, Some("http://new".to_string()));
        assert_eq!(entry.api_key, Some("old_key".to_string()));
        assert_eq!(entry.model, Some("old_model".to_string()));
    }

    #[test]
    fn test_resolve_provider_config_cli_overrides() {
        let mut cfg = Config::default();
        cfg.provider.default = Some("ollama_cloud".to_string());
        cfg.provider.entries.insert(
            "ollama_cloud".to_string(),
            ProviderEntry {
                host: Some("http://custom".to_string()),
                api_key: Some("config_key".to_string()),
                model: Some("config_model".to_string()),
            },
        );

        let (name, model, host, key) = resolve_provider_config(
            &cfg,
            Some("openai"),
            Some("gpt-4o"),
            Some("cli_key"),
        );
        assert_eq!(name, "openai");
        assert_eq!(model, "gpt-4o");
        assert_eq!(host, Some("https://api.openai.com".to_string()));
        assert_eq!(key, Some("cli_key".to_string()));
    }

    #[test]
    fn test_resolve_provider_config_defaults() {
        let cfg = Config::default();
        let (name, model, host, key) = resolve_provider_config(
            &cfg, None, None, None,
        );
        assert_eq!(name, "ollama_cloud");
        assert_eq!(model, "qwen2.5-coder:7b");
        assert_eq!(host, Some("https://api.ollama.com".to_string()));
        assert_eq!(key, None);
    }
}
