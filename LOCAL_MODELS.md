# AGCLI Local Models

AGCLI follows a **two-mode provider architecture**:

1. **Local mode** (Ollama/local server)
2. **Cloud mode** (OpenAI-compatible API gateway)

Routing is automatic with **local-first autodetection**.

## Local mode setup

```bash
export AGCLI_LOCAL_PROVIDER=ollama
export AGCLI_LOCAL_BASE_URL=http://127.0.0.1:11434
```

`OLLAMA_HOST` is also supported as a local base-URL signal.

## Local model selection

```bash
agcli --model ollama/qwen2.5-coder:7b
agcli --model ollama/llama3.2
```

If the `ollama/` prefix is omitted, AGCLI can still route locally when local env signals are present.

## Cloud gateway fallback

Cloud mode uses OpenAI-compatible credentials:

```bash
export OPENAI_API_KEY="your-key"
# optional:
export OPENAI_BASE_URL="https://your-gateway.example/v1"
```

This cloud path is also how you target compatible gateways, including Copilot-compatible and Gemini-compatible endpoints.

## Autodetection order

1. Explicit local model prefix (`ollama/`, `local/`)
2. Local env (`AGCLI_LOCAL_PROVIDER`, `OLLAMA_HOST`)
3. Explicit cloud model prefix (`openai/`, `gpt-`)
4. Cloud env (`OPENAI_API_KEY`)
5. Fallback to local mode
