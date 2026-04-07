# MASK (Mesh Agent Shared Knowledge)

**MASK** is the Shared Intelligence Layer (Pillar 4) of the local **Rust Agent Infrastructure (RAI)** ecosystem. 

It provides a decentralized, peer-to-peer Retrieval-Augmented Generation (RAG) system for multi-agent autonomous coding systems. Instead of loading the entire repository into every agent's context window—which is slow and expensive—MASK enables dynamic context embedding and retrieval with **zero external API calls**.

## Architecture

MASK operates in two modes:

1. **The Indexer (`mask serve`)**: One agent runs the indexer. It parses a local project directory, splits code files into logical chunks, calculates vector embeddings purely locally using `fastembed`, stores them in memory, and exposes an HTTP API (`axum`). Crucially, it registers its presence on the local network via `camp` (mDNS) under the role `knowledge-base`.
2. **The Query Worker (`mask query`)**: Other LLM agents in the system query MASK without needing to know its IP or ports. `mask query` interrogates the `camp` mesh to find the active Indexer, transmits the query, and retrieves top-k code snippets ranked by cosine similarity.

## Installation

Assuming you have Rust installed locally:

```bash
cargo install --path .
```

*Note: MASK requires the `coding_agent_mesh_presence` crate to be available in the workspace or resolving environment to utilize the LAN mesh discovery.*

## Usage

### 1. Start the Indexer (Server)

Run the server on a codebase target. This will take a few moments to automatically download the embedding models on first run and index the files:

```bash
mask serve ./src
```
*The server will block and remain active, listening for query requests and broadcasting via mDNS.*

### 2. Query the Knowledge Base (Client)

In another terminal, an agent can ask a question:

```bash
mask query "How does authentication work?"
```

**Output Contract:**
MASK is an Agent-Computer Interface (ACI). To ensure flawless tool consumption by LLMs, success `stdout` strictly emits a JSON array with no decorators:

```json
[
  {
    "file_path": "src/auth.rs",
    "content": "pub fn verify_token() -> Result<()> { ... }",
    "similarity_score": 0.892
  },
  {
    "file_path": "src/handlers.rs",
    "content": "let token = req.headers().get(\"Authorization\");",
    "similarity_score": 0.745
  }
]
```

## Security & Privacy
- **Zero API Keys**: Uses `fastembed-rs` pointing to local models. No code is transmitted to OpenAI, Anthropic, or external embedding providers.
- **Zero Docker/DB dependencies**: Avoids PostgreSQL or Redis. Vector similarity searches are heavily optimized in-memory computations, minimizing overhead for local AI swarms. 
