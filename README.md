# code-ant

Autonomous code-improvement agent for the command line.

`code-ant` crawls your git repository, suggests safe micro-improvements (unused imports, missing docstrings, type hints, lint fixes, antipatterns), and commits each change individually — validated by your test suite.

## Installation

### Homebrew

```bash
brew tap helloodokai/code-ant
brew install code-ant
```

### From source

```bash
cargo build --release
```

## Configuration

`code-ant` reads configuration from two optional TOML files:

1. **User-level**: `~/.code-ant/config.toml`
2. **Project-level**: `./.code-ant/config.toml`

Project-level config overrides user-level config. CLI flags and environment variables override both.

### Example `config.toml`

```toml
[provider]
default = "anthropic"

[provider.anthropic]
api_key = "sk-ant-api03-..."
model = "claude-sonnet-4-6"

[provider.openai]
api_key = "sk-proj-..."
model = "gpt-4"

[provider.ollama_local]
host = "http://localhost:11434"
model = "qwen2.5-coder:32b"
```

### Environment variables (highest priority)

- `CODE_ANT_PROVIDER`
- `CODE_ANT_MODEL`
- `CODE_ANT_API_KEY`

## Usage

```bash
# Run with defaults (auto-detects test suite)
code-ant

# Limit to 5 commits
code-ant --max-commits 5

# Only Python files, skip test verification
code-ant --include "**/*.py" --skip-tests

# Custom model provider
code-ant --provider anthropic --model claude-sonnet-4-6 --api-key $ANTHROPIC_API_KEY
```

## Environment variables

- `CODE_ANT_PROVIDER` — override provider (`ollama_cloud`, `ollama_local`, `openai`, `anthropic`)
- `CODE_ANT_MODEL` — override model name
- `CODE_ANT_API_KEY` — override API key

## Supported model providers

- **Ollama Cloud** (default)
- **Ollama Local**
- **OpenAI**
- **Anthropic**

## License

MIT
