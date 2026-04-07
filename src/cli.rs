use std::path::PathBuf;

use clap::{Parser, Subcommand};

pub const DEFAULT_PORT: u16 = 7_841;
pub const DEFAULT_DISCOVERY_WAIT_MS: u64 = 800;
pub const DEFAULT_TOP_K: usize = 5;

#[derive(Debug, Parser)]
#[command(
    name = "mask",
    version,
    about = "Shared intelligence layer for code-aware agent retrieval"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Index a directory, announce it on the mesh, and serve query requests.
    Serve(ServeConfig),
    /// Discover a knowledge-base peer on the mesh and emit strict JSON results.
    Query(QueryConfig),
}

#[derive(Debug, Clone, Parser)]
pub struct ServeConfig {
    /// Directory to recursively index.
    pub directory: PathBuf,
    /// TCP port to bind and advertise on the local mesh.
    #[arg(long, default_value_t = DEFAULT_PORT)]
    pub port: u16,
    /// Comma-separated list of file extensions to include (e.g. "rs,md,txt").
    #[arg(long, value_delimiter = ',', default_value = "rs,md,txt,toml,json")]
    pub extensions: Vec<String>,
    /// Maximum number of lines per semantic chunk.
    #[arg(long, default_value_t = 40)]
    pub chunk_lines: usize,
    /// Maximum number of characters per semantic chunk.
    #[arg(long, default_value_t = 2000)]
    pub chunk_chars: usize,
}

#[derive(Debug, Clone, Parser)]
pub struct QueryConfig {
    /// Natural-language question to ask the shared knowledge base.
    pub question: String,
    /// Number of best-matching chunks to request.
    #[arg(long, default_value_t = DEFAULT_TOP_K)]
    pub top_k: usize,
    /// How long to wait for CAMP discovery before selecting a peer.
    #[arg(long, default_value_t = DEFAULT_DISCOVERY_WAIT_MS)]
    pub discover_ms: u64,
}

pub fn parse() -> Result<Cli, clap::Error> {
    Cli::try_parse()
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};

    use super::{Cli, Command, DEFAULT_DISCOVERY_WAIT_MS, DEFAULT_PORT, DEFAULT_TOP_K};
    use clap::Parser as _;

    #[test]
    fn parses_serve_command_with_defaults() -> Result<()> {
        let cli = Cli::try_parse_from(["mask", "serve", "./src"])?;

        match cli.command {
            Command::Serve(config) => {
                assert_eq!(config.directory.to_string_lossy(), "./src");
                assert_eq!(config.port, DEFAULT_PORT);
                Ok(())
            }
            Command::Query(_) => bail!("expected serve command"),
        }
    }

    #[test]
    fn parses_query_command_with_strict_json_knobs() -> Result<()> {
        let cli = Cli::try_parse_from([
            "mask",
            "query",
            "How does auth work?",
            "--top-k",
            "3",
            "--discover-ms",
            "1200",
        ])?;

        match cli.command {
            Command::Query(config) => {
                assert_eq!(config.question, "How does auth work?");
                assert_eq!(config.top_k, 3);
                assert_eq!(config.discover_ms, 1_200);
                Ok(())
            }
            Command::Serve(_) => bail!("expected query command"),
        }
    }

    #[test]
    fn query_defaults_stay_agent_friendly() -> Result<()> {
        let cli = Cli::try_parse_from(["mask", "query", "What changed?"])?;

        match cli.command {
            Command::Query(config) => {
                assert_eq!(config.top_k, DEFAULT_TOP_K);
                assert_eq!(config.discover_ms, DEFAULT_DISCOVERY_WAIT_MS);
                Ok(())
            }
            Command::Serve(_) => bail!("expected query command"),
        }
    }
}
