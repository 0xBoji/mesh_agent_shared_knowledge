use std::io::Write;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryRequest {
    pub query: String,
    pub top_k: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryResult {
    pub file_path: String,
    pub content: String,
    pub similarity_score: f32,
}

pub fn render_query_results(results: &[QueryResult]) -> Result<String> {
    serde_json::to_string(results).map_err(Into::into)
}

pub fn print_query_results(mut writer: impl Write, results: &[QueryResult]) -> Result<()> {
    writer.write_all(render_query_results(results)?.as_bytes())?;
    writer.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{QueryResult, render_query_results};

    #[test]
    fn renders_a_strict_json_array_without_extra_decorators() -> Result<()> {
        let json = render_query_results(&[QueryResult {
            file_path: "src/auth.rs".into(),
            content: "fn login() {}".into(),
            similarity_score: 0.98,
        }])?;

        assert_eq!(
            json,
            r#"[{"file_path":"src/auth.rs","content":"fn login() {}","similarity_score":0.98}]"#
        );
        Ok(())
    }

    #[test]
    fn renders_empty_results_as_empty_array() -> Result<()> {
        let json = render_query_results(&[])?;

        assert_eq!(json, "[]");
        Ok(())
    }
}
