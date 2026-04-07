use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use coding_agent_mesh_presence::{AgentInfo, ZeroConfMesh};
use tokio::net::TcpListener;
use tokio::time;

use crate::cli::{QueryConfig, ServeConfig};
use crate::indexer::{EmbeddingClient, IndexConfig, KnowledgeBase, build_index};
use crate::output::{QueryRequest, QueryResult};

#[derive(Clone)]
struct AppState {
    knowledge_base: Arc<KnowledgeBase>,
    embedder: EmbeddingClient,
}

impl AppState {
    fn new(knowledge_base: KnowledgeBase, embedder: EmbeddingClient) -> Self {
        Self {
            knowledge_base: Arc::new(knowledge_base),
            embedder,
        }
    }
}

pub async fn serve(config: ServeConfig) -> Result<()> {
    let index_config = IndexConfig {
        extensions: config.extensions.clone(),
        chunk_line_limit: config.chunk_lines,
        chunk_char_limit: config.chunk_chars,
    };
    let embedder = EmbeddingClient::new_fastembed().await?;
    let knowledge_base = build_index(&config.directory, &embedder, &index_config).await?;
    let chunk_count = knowledge_base.len();
    let state = AppState::new(knowledge_base, embedder);
    let router = build_router(state);

    let mesh = ZeroConfMesh::builder()
        .agent_id(format!("mask-indexer-{}", config.port))
        .role("knowledge-base")
        .project("mask")
        .branch("serve")
        .port(config.port)
        .build()
        .await
        .context("failed to announce knowledge-base service to CAMP")?;

    let bind_addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.port));
    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind HTTP server on {bind_addr}"))?;

    eprintln!(
        "mask serving {} chunks from {} on http://0.0.0.0:{}",
        chunk_count,
        config.directory.display(),
        config.port
    );

    let server_result = axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum server terminated unexpectedly");

    let shutdown_result = mesh
        .shutdown()
        .await
        .context("failed to shutdown CAMP mesh");

    server_result?;
    shutdown_result
}

pub async fn query_mesh(config: QueryConfig) -> Result<Vec<QueryResult>> {
    let base_url = discover_knowledge_base(Duration::from_millis(config.discover_ms)).await?;
    let client = reqwest::Client::new();
    let request = QueryRequest {
        query: config.question,
        top_k: config.top_k,
    };

    client
        .post(format!("{base_url}/query"))
        .json(&request)
        .send()
        .await
        .context("failed to contact discovered knowledge-base")?
        .error_for_status()
        .context("knowledge-base returned an HTTP error")?
        .json::<Vec<QueryResult>>()
        .await
        .context("knowledge-base returned invalid JSON")
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/query", post(query_handler))
        .with_state(state)
}

async fn query_handler(
    State(state): State<AppState>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<Vec<QueryResult>>, (StatusCode, String)> {
    let embeddings = state
        .embedder
        .embed_texts(vec![format!("query: {}", request.query)])
        .await
        .map_err(internal_error)?;

    let query_embedding = embeddings
        .into_iter()
        .next()
        .ok_or_else(|| internal_error(anyhow!("embedding model returned no query vector")))?;

    let results = state.knowledge_base.search(&query_embedding, request.top_k);
    Ok(Json(results))
}

async fn discover_knowledge_base(wait: Duration) -> Result<String> {
    let mesh = ZeroConfMesh::builder()
        .agent_id("mask-query-client")
        .role("query-client")
        .project("mask")
        .branch("query")
        .port(ephemeral_udp_port()?)
        .discover_only()
        .build()
        .await
        .context("failed to start CAMP discovery observer")?;

    time::sleep(wait).await;
    let agents = mesh.agents_by_role("knowledge-base").await;
    let discovery_result = select_knowledge_base_url(&agents);
    let shutdown_result = mesh
        .shutdown()
        .await
        .context("failed to shutdown discovery observer");

    let url = discovery_result?;
    shutdown_result?;
    Ok(url)
}

fn select_knowledge_base_url(agents: &[AgentInfo]) -> Result<String> {
    let agent = agents
        .iter()
        .find(|agent| !agent.addresses().is_empty())
        .ok_or_else(|| anyhow!("no knowledge-base peer discovered on the local mesh"))?;

    let address = pick_address(agent.addresses())
        .ok_or_else(|| anyhow!("discovered knowledge-base peer had no usable IP address"))?;

    Ok(format!("http://{}:{}", format_ip(address), agent.port()))
}

fn pick_address(addresses: &[IpAddr]) -> Option<IpAddr> {
    addresses
        .iter()
        .copied()
        .find(IpAddr::is_ipv4)
        .or_else(|| addresses.first().copied())
}

fn format_ip(address: IpAddr) -> String {
    match address {
        IpAddr::V4(address) => address.to_string(),
        IpAddr::V6(address) => format!("[{address}]"),
    }
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn ephemeral_udp_port() -> Result<u16> {
    let socket =
        UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).context("failed to allocate UDP port")?;
    let port = socket
        .local_addr()
        .context("failed to inspect allocated UDP port")?
        .port();
    Ok(port)
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use coding_agent_mesh_presence::{AgentAnnouncement, AgentMetadata, AgentStatus};

    use super::{AppState, format_ip, pick_address, query_handler};
    use crate::indexer::{Chunk, EmbeddingBackend, EmbeddingClient, KnowledgeBase};
    use crate::output::{QueryRequest, QueryResult};
    use axum::Json;
    use axum::extract::State;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    struct FakeBackend;

    impl EmbeddingBackend for FakeBackend {
        fn embed(&mut self, inputs: Vec<String>) -> Result<Vec<Vec<f32>>> {
            Ok(inputs
                .into_iter()
                .map(|text| {
                    if text.contains("auth") {
                        vec![1.0, 0.0]
                    } else {
                        vec![0.0, 1.0]
                    }
                })
                .collect())
        }
    }

    #[tokio::test]
    async fn query_handler_returns_top_matches_from_in_memory_state() -> Result<()> {
        let knowledge_base = KnowledgeBase::new(vec![
            Chunk {
                file_path: "src/auth.rs".into(),
                content: "auth login flow".into(),
                embedding: vec![1.0, 0.0],
            },
            Chunk {
                file_path: "src/db.rs".into(),
                content: "db pool setup".into(),
                embedding: vec![0.0, 1.0],
            },
        ]);
        let state = AppState::new(knowledge_base, EmbeddingClient::from_backend(FakeBackend));

        let Json(results) = query_handler(
            State(state),
            Json(QueryRequest {
                query: "How does auth work?".into(),
                top_k: 1,
            }),
        )
        .await
        .map_err(|(_, message)| anyhow::anyhow!(message))?;

        assert_eq!(
            results,
            vec![QueryResult {
                file_path: "src/auth.rs".into(),
                content: "auth login flow".into(),
                similarity_score: 1.0,
            }]
        );
        Ok(())
    }

    #[test]
    fn ip_formatting_prefers_ipv4_and_brackets_ipv6() {
        assert_eq!(
            pick_address(&[
                IpAddr::V6(Ipv6Addr::LOCALHOST),
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42)),
            ]),
            Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42)))
        );
        assert_eq!(format_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)), "[::1]");
    }

    #[test]
    fn discovery_url_can_be_derived_from_announced_addresses() -> Result<()> {
        let announcement = AgentAnnouncement::new(
            "mask._mesh._tcp.local.",
            "mask-1",
            "knowledge-base",
            "mask",
            "serve",
            AgentStatus::Idle,
            7841,
            vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))],
            AgentMetadata::new(),
        )?;

        assert_eq!(announcement.port(), 7_841);
        assert_eq!(format_ip(announcement.addresses()[0]), "10.0.0.5");
        Ok(())
    }
}
