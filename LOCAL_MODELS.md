# AGCLI Local Models

AGCLI is being oriented toward local-first model execution.

## First target

- `Ollama`

The new local provider lives in the Rust `api` crate and speaks to Ollama's native `/api/chat` endpoint.

## Environment

```bash
export AGCLI_LOCAL_PROVIDER=ollama
export AGCLI_LOCAL_BASE_URL=http://127.0.0.1:11434
```

`OLLAMA_HOST` is also supported as a fallback for the base URL.

## Model selection

You can pass the model directly, for example:

```bash
agcli --model ollama/qwen2.5-coder:7b
agcli --model ollama/llama3.2
```

If the `ollama/` prefix is omitted, AGCLI can still route locally when:

- `AGCLI_LOCAL_PROVIDER=ollama` is set, or
- `OLLAMA_HOST` is present in the environment

## Current limitation

This provider currently focuses on local chat/streaming transport.
Tool-calling parity with cloud-native providers still needs more work at the agent loop level.
