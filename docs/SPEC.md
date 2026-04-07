You are an expert Rust systems engineer and AI infrastructure architect. I am building a tool called `mesh_agent_shared_knowledge` (CLI command: `mask`) — the Shared Intelligence Layer (Pillar 4) of my local Rust Agent Infrastructure (RAI) ecosystem.

## Context & Problem

In multi-agent autonomous coding, having every agent read the entire repository into its context window is slow, wastes LLM tokens, and causes context bloat. MASK solves this with a decentralized, peer-to-peer RAG system:

- An **Indexer agent** runs `mask serve ./src` — it chunks local code files, generates embeddings locally, starts a lightweight HTTP API, and broadcasts its presence via the local LAN using the `coding_agent_mesh_presence` (CAMP) crate with `role=knowledge-base`.
- **Worker agents** run `mask query "How does authentication work?"` — this uses CAMP to auto-discover the Indexer's IP/port, sends the query, and retrieves the most relevant code snippets as structured JSON.

## Tech Stack

| Concern | Crate |
|---|---|
| CLI | `clap` (derive feature) |
| Async runtime | `tokio` |
| HTTP server | `axum` |
| HTTP client | `reqwest` |
| Local embeddings | `fastembed` (pure-Rust, no Python, no OpenAI) |
| Vector store | In-memory cosine similarity (`Vec<Chunk>`) for MVP — no Docker, no external DBs |
| Mesh discovery | `coding_agent_mesh_presence` (assume this local crate exists; use `ZeroConfMesh::builder().role("knowledge-base").port(port).build()` for announcement and its discovery API for querying) |

## Required Implementation

Produce a complete, production-ready MVP with the following modules:

**`cli.rs`**
- `mask serve <directory>` — starts the indexer and server
- `mask query <"question">` — discovers the server via CAMP mesh and executes the query

**`indexer.rs`**
- Recursively reads all text/code files from the given directory
- Splits files into naive chunks (by double newlines or fixed line count)
- Uses `fastembed` to generate vector embeddings for each chunk
- Stores chunks in memory as `Vec<Chunk>`
- Add inline comments walking through the full RAG pipeline: chunking → embedding → cosine similarity

**`server.rs`**
- Exposes a `POST /query` endpoint via `axum`
- On startup, announces to the mesh via `ZeroConfMesh::builder().role("knowledge-base").port(port).build()`
- Accepts a query string, embeds it, runs cosine similarity against the in-memory store, returns top-k results

**`output.rs`**
- The `mask query` command MUST emit a strict, clean JSON array to stdout — no extra logging, no decorators
- Each object in the array contains: `file_path`, `content`, `similarity_score`
- This is non-negotiable — it must be safe for LLM tool-calling and pipe-friendly

## Deliverables

1. **`Cargo.toml`** — exact dependency versions for all crates listed above, including `fastembed` and `axum`
2. **Project structure** — module layout with brief description of each file's responsibility
3. **Full implementation** — complete, compiling code for all modules with inline comments explaining the RAG pipeline and CAMP mesh integration
4. **Example usage** — shell commands showing one agent running the server and another querying it, with the expected JSON output format

## Code Quality Requirements

- **Panic-free** — use `Result` and `?` propagation throughout; no `.unwrap()` in production paths
- **Zero-config** — works out of the box with no environment variables, config files, or external services
- **ACI-optimized** — treat this as an Agent-Computer Interface; the JSON output contract is the primary integration surface and must be exact
- **Async-first** — use `tokio` throughout; blocking operations must be explicitly wrapped
- Write this as if it will be extended to Pillar 5 — keep module boundaries clean and avoid coupling the indexer, server, and CLI logic