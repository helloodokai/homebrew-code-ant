use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformCandidate {
    pub file_path: String,
    pub description: String,
    pub diff: String,
    pub new_content: String,
}

pub struct Transformer {
    provider: crate::models::ModelProvider,
}

impl Transformer {
    pub fn new(provider: crate::models::ModelProvider) -> Self {
        Transformer { provider }
    }

    pub async fn generate_candidates(
        &self,
        file_path: &str,
        content: &str,
        language: &str,
    ) -> Result<Vec<TransformCandidate>> {
        let system = "You are an expert code-improvement agent. You apply ONLY safe, incremental, non-breaking improvements. You NEVER change observable behavior, public API signatures, or fix security issues. You output valid JSON only.";
        let prompt = format!(
            "Analyze the following {} file and suggest up to 3 safe micro-improvements.\n\
            Allowed improvements: remove unused imports, add type hints, fix lint violations, add missing docstrings, replace known antipatterns.\n\
            Do NOT: refactor architecture, change public APIs, or address security vulnerabilities.\n\
            For each improvement, provide a concise description, a unified diff, and the full new file content.\n\
            If no improvements are possible, return an empty array.\n\
            File: {}\n\n```{}\n{}\n```\n\n\
            Respond with valid JSON in this exact schema:\n\
            [\n\
              {{\"description\": \"concise description\", \"diff\": \"unified diff string\", \"new_content\": \"full new file content\"}}\n\
            ]",
            language, file_path, language, content
        );

        let response = self.provider.generate(system, &prompt).await?;
        let candidates = parse_response(&response, file_path)?;
        Ok(candidates)
    }
}

fn parse_response(response: &str, file_path: &str) -> Result<Vec<TransformCandidate>> {
    let cleaned = response.trim();
    let json_str = if cleaned.starts_with("```") {
        let start = cleaned.find('\n').unwrap_or(0) + 1;
        let end = cleaned.rfind("```").unwrap_or(cleaned.len());
        &cleaned[start..end]
    } else {
        cleaned
    };

    let trimmed = json_str.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return Ok(Vec::new());
    }

    let items: Vec<serde_json::Value> = serde_json::from_str(trimmed)?;
    let mut candidates = Vec::new();
    for item in items {
        let description = item
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let diff = item
            .get("diff")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let new_content = item
            .get("new_content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !description.is_empty() && !new_content.is_empty() {
            candidates.push(TransformCandidate {
                file_path: file_path.to_string(),
                description,
                diff,
                new_content,
            });
        }
    }
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ModelProvider;

    #[tokio::test]
    async fn test_transformer_parse_response_empty() {
        let provider = ModelProvider::Mock {
            response: "[]".to_string(),
        };
        let t = Transformer::new(provider);
        let candidates = t.generate_candidates("x.py", "x = 1\n", "py").await.unwrap();
        assert!(candidates.is_empty());
    }

    #[tokio::test]
    async fn test_transformer_parse_response_valid() {
        let provider = ModelProvider::Mock {
            response: r#"[{"description": "Remove unused import", "diff": "", "new_content": "import sys\nprint(sys.version)\n"}]"#.to_string(),
        };
        let t = Transformer::new(provider);
        let candidates = t.generate_candidates("a.py", "import os\nimport sys\nprint(sys.version)\n", "py").await.unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].description, "Remove unused import");
        assert!(candidates[0].new_content.contains("import sys"));
        assert!(!candidates[0].new_content.contains("import os"));
    }
}
