use std::process::Command;

use anyhow::Result;

#[test]
fn query_without_a_discovered_server_keeps_stdout_machine_readable() -> Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_mask"))
        .args(["query", "What is auth?", "--discover-ms", "1"])
        .output()?;

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "[]\n");
    assert!(!output.stderr.is_empty());
    Ok(())
}
