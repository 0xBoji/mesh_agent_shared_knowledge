# AGENTS.md — mesh_agent_shared_knowledge

This file governs the entire `mesh_agent_shared_knowledge/` repository.

## Mission
Build `mask` (Mesh Agent Shared Knowledge), a decentralized, zero-config RAG (Retrieval-Augmented Generation) and Vector DB layer that powers local context sharing between AI coding agents.

The source of truth is `docs/SPEC.md`.

## Product contract
- Binary name: `mask`
- Primary commands:
  - `mask serve <directory>`: Runs the Indexer agent. Embeds code files locally and starts a lightweight `axum` HTTP server while announcing on the local LAN via the `camp` mesh layer (`role=knowledge-base`).
  - `mask query <"question">`: Discovers the active Indexer via CAMP, submits the query, and emits results.
- Core restrictions:
  - **Zero external APIs**: Embeddings must be purely local (e.g. `fastembed`), keeping token costs at zero and bypassing rate limits/API keys.
  - **No heavy DBs**: Vector similarity (Cosine Similarity) must be done in-memory. Do not introduce Postgres/Redis/Docker dependencies.
  - **Decentralized**: Agents must discover the Indexer dynamically via mDNS (using `coding_agent_mesh_presence`).

## Required technical choices
- `clap` v4 for CLI parsing
- `tokio` for async runtime
- `axum` for HTTP API server
- `reqwest` for the query client
- `fastembed` for pure-Rust, local embeddings
- `serde` and `serde_json` for strictly structured output
- `anyhow` for application-level error handling

## Expected module layout
Keep the crate modular and functional:
- `src/main.rs` — CLI parser and async entrypoint
- `src/cli.rs` — clap definitions and validations
- `src/indexer.rs` — file tree traversal, chunking, fastembed generation, in-memory chunk storage
- `src/server.rs` — `axum` server, CAMP mDNS announcement, and `query_mesh` client bridging
- `src/output.rs` — strict JSON formatting

## Output contract for AI Agents
`mask query` is an **Agent-Computer Interface (ACI)**. Its output is ingested directly by LLMs.
- Stdout must strictly emit a JSON array: `[{file_path, content, similarity_score}]`.
- Do not emit decorators, debug logs, progress bars, or human-readable fallback on `stdout`.
- If an error occurs, it should be emitted to `stderr`, and `stdout` must remain clean or output an empty array `[]`.

## Code quality rules
- **Panic-free**: No `unwrap()`, `expect()`, or `panic!` macros in production execution paths.
- Async block boundaries must not accidentally block the tokio thread; avoid heavy sync blocking operations in HTTP handlers.
- **Dependency discipline**: Do not add deps without strict justification. Maintain zero configuration defaults.

## Commit and agent-knowledge rules
- Treat git history as part of the agent memory for this repo.
- Every meaningful change should be committed with a Conventional Commit style subject: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`.
- For non-trivial commits, include `Constraint:`, `Rejected:`, `Confidence:`, `Scope-risk:`, `Tested:`, `Not-tested:` lore-style trailers.
- Do not batch unrelated changes into one commit.
