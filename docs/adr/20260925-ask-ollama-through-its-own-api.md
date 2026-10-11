# Ask Ollama through its own API

Status: Accepted

## Context

Ollama loads a model with the longest context the model takes unless the
request says otherwise. For a small model made for long contexts that is
tens of gigabytes of memory for a request that needs a fraction of it. Only
Ollama's own API takes the context length with each request.

## Decision

Ollama is asked through its own chat API, with the context sized to the
request. Other servers are asked through the OpenAI-compatible API.

## Alternatives considered

- Treat Ollama as one more OpenAI-compatible server. That API cannot size
  the context, so every request would load the model at its largest.

## Consequences

- Ollama is an agent of its own, beside `openai`.
- Arto, not Ollama, decides how much context a request loads.
