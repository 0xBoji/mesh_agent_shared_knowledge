mod cli;
mod indexer;
mod output;
mod server;

use std::io::Write as _;
use std::process::ExitCode;

use anyhow::Result;
use cli::{Command, QueryConfig};

#[tokio::main]
async fn main() -> ExitCode {
    match try_main().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "{error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn try_main() -> Result<()> {
    let cli = match cli::parse() {
        Ok(cli) => cli,
        Err(error) => {
            error.print()?;
            return Ok(());
        }
    };

    match cli.command {
        Command::Serve(config) => server::serve(config).await,
        Command::Query(config) => run_query(config).await,
    }
}

async fn run_query(config: QueryConfig) -> Result<()> {
    match server::query_mesh(config).await {
        Ok(results) => output::print_query_results(&mut std::io::stdout(), &results),
        Err(error) => {
            output::print_query_results(&mut std::io::stdout(), &[])?;
            Err(error)
        }
    }
}
