//! Client-side admin request used by the MCP bridge (one request, one
//! reply line, time-bounded).

use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::{json, Value};

/// Send one admin request over the daemon plane.
pub fn admin_request(method: &str, params: Value) -> Result<Value> {
    let mut stream = crate::daemon::connect_admin(Duration::from_secs(30))?;
    let request = json!({"id": 1, "method": method, "params": params});
    use std::io::{BufRead, BufReader, Write};
    writeln!(stream, "{request}")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if line.is_empty() {
        bail!("daemon closed the connection without a reply");
    }
    let response: Value = serde_json::from_str(line.trim())?;
    Ok(response)
}
