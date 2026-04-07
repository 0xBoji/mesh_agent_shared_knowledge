# MASK (Mesh Agent Shared Knowledge)

**MASK** is the **Shared Intelligence Layer (Pillar 4)** of the local **Rust Agent Infrastructure (RAI)** ecosystem.

MASK provides a decentralized, peer-to-peer Retrieval-Augmented Generation (RAG) system built exclusively for multi-agent autonomous coding swarms. Instead of eagerly loading the entire repository into every agent's LLM context window—which is catastrophically slow and expensive—MASK enables dynamic codebase querying via zero-config local mesh discovery.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](#installation)
[![crates.io](https://img.shields.io/crates/v/mesh_agent_shared_knowledge.svg)](https://crates.io/crates/mesh_agent_shared_knowledge)

---

## 📖 Table of Contents

- [The Core Problem](#the-core-problem)
- [How MASK Solves It](#how-mask-solves-it)
- [Architecture & Mechanics](#architecture--mechanics)
- [Installation](#installation)
- [Quickstart Guide](#quickstart-guide)
  - [1. Running the Indexer Node](#1-running-the-indexer-node)
  - [2. Querying the Node](#2-querying-the-node)
- [The Agent-Computer Interface (ACI)](#the-agent-computer-interface-aci)
- [Integration Examples](#integration-examples)
- [Security & Privacy](#security--privacy)
- [Local Development & Testing](#local-development--testing)
- [Roadmap](#roadmap)

---

## 🚧 The Core Problem

As autonomous coding swarms grow, the primary bottleneck stops being raw LLM intelligence and starts being **context management**.

1. **Token Bloat:** Sending a 50,000-line Rust repository to Claude or GPT-4 for every single file edit wastes enormous amounts of money.
2. **Attention Dilution:** Huge context windows cause models to hallucinate or miss minor details in the middle of prompts ("lost in the middle" phenomenon).
3. **Synchronization Overheads:** Setting up Postgres, pgvector, or Redis instances locally just to give an agent a Vector DB adds friction and breaks the "clone-and-go" philosophy of RAI.

---

## 🛠️ How MASK Solves It

MASK is a pure-Rust, zero-configuration RAG engine built on two pillars:

1. **`fastembed`**: Vector embeddings are calculated using highly optimized localized ONNX models. No Python, no heavy PyTorch dependencies, and **zero API calls to OpenAI/Anthropic**.
2. **`camp` (Coding Agent Mesh Presence)**: MASK automatically broadcasts its location over the local area network using mDNS. Searching agents do not need IP addresses, ports, or `.env` files. They simply ask the mesh: *"Who is the knowledge-base?"* and MASK answers.

---

## 🏗️ Architecture & Mechanics

MASK operates using two opposing execution flows encapsulated in a single binary:

### The Server (Indexer)
When you run `mask serve <directory>`, the following happens asynchronously:
1. **File Walk:** Recursively reads all text/code files inside the target directory.
2. **Chunking/Splitting:** Code is split intelligently into context-rich blocks.
3. **Embedding Generation:** Each block is converted into a high-dimensional vector locally.
4. **In-Memory Store:** Instead of a complex database, chunks and vectors are stored in a heavily optimized memory matrix for Cosine Similarity calculations.
5. **Mesh Announcement:** An `axum` HTTP server boots up, and `camp` is invoked to announce `role = knowledge-base` on the LAN.

### The Client (Worker)
When a secondary agent runs `mask query "How does X work?"`:
1. **Mesh Discovery:** The CLI queries the `camp` daemon to find active knowledge-bases.
2. **Forwarding:** The semantic text query is sent over HTTP via `reqwest` to the active indexer.
3. **Semantic Search:** The indexer embeds the query and calculates the cosine similarity against the chunk matrix.
4. **Structured JSON Emission:** The top relevant code snippets are returned and printed to `{stdout}` as a pristine JSON array, expressly formatted for LLM tool consumption.

---

## 💿 Installation

You can install MASK via the standard Cargo package manager, or use our specialized curl/bash installer.

**From crates.io (Standard path):**
```bash
cargo install mesh_agent_shared_knowledge --bin mask
```

**Using the Installer (Recommended for raw machine setup):**
```bash
bash <(curl -fsSL https://raw.githubusercontent.com/0xBoji/mesh_agent_shared_knowledge/main/scripts/install.sh)
```
*(You can pass `--git` to force building from the main GitHub branch).*

**Local development compilation:**
```bash
## Assumes `coding_agent_mesh_presence` is present in the parent workspace!
cargo install --path . --bin mask
```

---

## 🚀 Quickstart Guide

Using MASK is designed to require absolute zero boilerplate. No `.env` formats, no Docker containers.

### 1. Running the Indexer Node

Navigate to any large repository or folder you want the swarm to understand, and serve it:

```bash
mask serve ./src
```
**What you will see:**
The first time this runs, it will briefly download the local embedding ONNX models (cached permanently in `~/.cache`). Subsequent runs are near-instantaneous. The node will print its bound port and block the thread, waiting to serve queries.

### 2. Querying the Node

From any other terminal session, folder, or machine on the same LAN (that has `mask` and `camp` configured):

```bash
mask query "How do I authenticate with the CAMP mesh?"
```

MASK will output the semantically matched code segments bridging natural language with the exact implementation inside your codebase.

---

## 🤖 The Agent-Computer Interface (ACI)

MASK is explicitly not built for human readability. It is built as an **Agent-Computer Interface**. When an LLM executes the `query` command, it expects strict, parseable syntax.

`stdout` will *only* contain a properly formatted JSON array. Log messages, errors, and warnings are exclusively piped to `stderr`.

**Output Contract:**
```json
[
  {
    "file_path": "./src/authenticator.rs",
    "content": "pub fn verify_token(req: &HeaderMap) -> Result<()> { ... }",
    "similarity_score": 0.892
  },
  {
    "file_path": "./src/router.rs",
    "content": "let token = req.headers().get(\"Authorization\");",
    "similarity_score": 0.745
  }
]
```

**Why this matters:**
Your automated Agent workflows can safely do `result = $(mask query "auth")` and immediately parse `result` with `JSON.parse()` without worrying about ASCII banners or verbose logging destroying the JSON parser.

---

## 🔌 Integration Examples

Because MASK implements Unix-philosophy streams properly, embedding it into custom agentic Python or Node.js scripts is trivial.

### Calling from Python
```python
import subprocess
import json

def get_code_context(question: str) -> list:
    cmd = ["mask", "query", question]
    # mask explicitly emits clean JSON to stdout
    result = subprocess.run(cmd, capture_output=True, text=True, check=True)
    return json.loads(result.stdout)

context = get_code_context("Where is the database connection pool initialized?")
for snippet in context:
    print(f"Look at {snippet['file_path']} (score: {snippet['similarity_score']})")
```

### Calling from a bash pipeline
Filter out components with `jq`:
```bash
mask query "wasi preview1 imports" | jq '.[0].file_path'
```

---

## 🔒 Security & Privacy

MASK operates completely “off the grid”.
- **Zero API Keys**: Uses `fastembed` pointing to local machine models.
- **Data Privacy**: Source Code is never transmitted to OpenAI, Anthropic, Google, or external embedding providers. Everything operates inside your machine's CPU/RAM.
- **Zero Heavy Infrastructure**: Bypasses the need for PostgreSQL or Redis. Vector similarity searches are highly optimized in-memory array computations leveraging Rust's bare-metal speed.

---

## 🏗️ Local Development & Testing

Before submitting PRs to this project, ensure you respect the strict workspace consistency limits.

```bash
# Ensure strict formatting
cargo fmt --all

# Run clippy with maximum warnings
cargo clippy --all-targets --all-features -- -D warnings

# Execute test suite (verifies 100% of the JSON output purity contracts and embedding pipelines)
cargo test
```

Please refer to `AGENTS.md` for our strict, local project lore and Git commit standards before merging. 

---

## 🗺️ Roadmap

- [x] Initial CLI setup with Subcommands
- [x] In-memory file chunking & FastEmbed generation
- [x] Cosine similarity ranking
- [x] ZeroConfMesh / CAMP mDNS announcement
- [x] Strict JSON output purity tests
- [ ] Adaptive chunking sizes via CLI (`--chunk-size 500`)
- [ ] File extension filtering (`--include "*.rs,*.md"`)
- [ ] Continuous hot-reloading (re-indexing files if they change on disk)
