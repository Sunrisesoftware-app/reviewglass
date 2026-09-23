# ADR-0023: The explain backends: the Claude Messages API with the user's own key, and the OpenAI-compatible chat protocol on loopback

**Status:** Accepted
**Atlas id:** `adr.rg.023`
**Links:** `rg.explain-service` (Explain service (novice mode)), `rg.explain-backend` (Explain backend (trait, two implementations)), `ext.explain-remote` (Remote explain API), `ext.local-compute` (Local compute unit (127.0.0.1)), `reviewglass`

> Rendered from the Atlas model (system `reviewglass`). The model is the source of
> truth; edit it there and run `node scripts/adr-from-model.mjs`.

---

## Context

P6 comes next by the owner's decision of 23.9.2026, and adr.rg.002 set two equal-status backends behind one interface without naming their protocols. The owner asked for a backend of one's own key (BYOK), and later a watcher, the hawk eye, standing on it. A local compute unit on this machine may run Ollama, LM Studio or llama.cpp's server, all of which answer the OpenAI-compatible chat completions protocol. The spec proposed a 30 s timeout and cancellation from the UI; a current Claude model thinks before it answers, and a non-streaming request that is only abandoned by the UI still runs, and is billed, to its end.

## Decision

RemoteBackend speaks the Claude Messages API (POST https://api.anthropic.com/v1/messages, anthropic-version 2023-06-01) with the user's own API key, stored only in Windows Credential Manager and sent only in the x-api-key header. The model is a setting whose default is claude-opus-5, at output effort medium, with max_tokens 16000 and the server-side refusal fallback (fallbacks "default", beta server-side-fallback-2026-07-01); a refusal is shown as a refusal, never as an error. LocalBackend speaks the OpenAI-compatible chat completions protocol (POST /v1/chat/completions) on a loopback host only (127.0.0.0/8, ::1 or localhost), with a model name the user sets. Both are asynchronous: one request at a time, a 60 s timeout, cancellation from the UI aborts the request itself (the connection is dropped, not merely ignored), and neither retries. The prompt is built by explain-service, provider-agnostic; each backend only maps it to its wire format, and the first remote request of an installation shows the exact URL, headers (the key withheld) and body that will leave the machine before anything is sent.

## Consequences

The app gains an HTTP client (reqwest over the Windows TLS stack, native-tls/SChannel: the system's certificate store and proxy, no new root store). A provider name appears only under src-tauri/src/explain/backend. The 30 s the spec proposed becomes 60 s, since the default model thinks before it answers. A third backend is one more implementation in that directory and nothing else. The hawk eye (P6b) will stand on the same interface but not on this consent: a continuous send needs its own decision.
