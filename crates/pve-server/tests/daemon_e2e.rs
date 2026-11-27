//! Daemon lifecycle e2e: lock mutual exclusion (sequential + concurrent),
//! stale-lock takeover (zombie PID correctness), admin-plane roundtrip,
//! and the stop lifecycle. Each test runs in its own nextest process with
//! an isolated port dir.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Result};

/// A minimal valid config: host points at loopback (never contacted by
/// status/ping/stop), fake token pair satisfies validation.
const TEST_CONFIG: &str = r#"
[pve]
host = "127.0.0.1"
token_id = "root@pam!test"
token_value = "test-secret"
"#;

struct TestEnv {
    _dir: tempfile::TempDir,
    port_dir: PathBuf,
    config_path: PathBuf,
}

impl TestEnv {
    fn new() -> Result<Self> {
        let dir = tempfile::tempdir()?;
        let port_dir = dir.path().join("portdir");
        std::fs::create_dir_all(&port_dir)?;
        let config_path = dir.path().join("pve-mcp.toml");
        std::fs::write(&config_path, TEST_CONFIG)?;
        Ok(Self {
            _dir: dir,
            port_dir,
            config_path,
        })
    }
}

fn spawn_daemon(env: &TestEnv) -> Result<std::process::Child> {
    let exe = env!("CARGO_BIN_EXE_pve-mcp");
    std::process::Command::new(exe)
        .arg("--config")
        .arg(&env.config_path)
        .arg("--port-dir")
        .arg(&env.port_dir)
        .arg("daemon")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("spawning daemon binary")
}

use anyhow::Context;

fn wait_ready(env: &TestEnv, timeout: Duration) -> Result<u16> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if std::time::Instant::now() > deadline {
            bail!("daemon did not become ready in time");
        }
        let port_file = env.port_dir.join("admin.port");
        if let Ok(text) = std::fs::read_to_string(&port_file) {
            if let Ok(port) = text.trim().parse::<u16>() {
                // Confirm it actually accepts.
                if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                    return Ok(port);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn admin_request(port: u16, method: &str) -> Result<serde_json::Value> {
    use std::io::{BufRead, BufReader, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let request = serde_json::json!({"id": 1, "method": method, "params": {}});
    writeln!(stream, "{request}")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    anyhow::ensure!(!line.is_empty(), "no reply from admin plane");
    Ok(serde_json::from_str(line.trim())?)
}

fn wait_stopped(env: &TestEnv, timeout: Duration) -> Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if std::time::Instant::now() > deadline {
            bail!("daemon did not stop in time");
        }
        let port_file = env.port_dir.join("admin.port");
        let connectable = std::fs::read_to_string(&port_file).ok().and_then(|text| {
            text.trim()
                .parse::<u16>()
                .ok()
                .map(|port| std::net::TcpStream::connect(("127.0.0.1", port)).is_ok())
        });
        if connectable != Some(true) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn daemon_starts_admin_roundtrip_and_stops() -> Result<()> {
    let env = TestEnv::new()?;
    let mut child = spawn_daemon(&env)?;
    let port = wait_ready(&env, Duration::from_secs(15)).context("waiting ready")?;

    let status = admin_request(port, "status")?;
    assert_eq!(status["error"], serde_json::Value::Null);
    assert_eq!(status["result"]["version"], "0.1.0");
    assert!(status["result"]["pid"].as_u64().is_some());

    let ping = admin_request(port, "ping")?;
    assert_eq!(ping["result"]["pong"], true);

    let stop = admin_request(port, "stop")?;
    assert_eq!(stop["result"]["stopping"], true);
    wait_stopped(&env, Duration::from_secs(10))?;
    let _ = child.kill();
    Ok(())
}

#[test]
fn lock_rejects_second_daemon_while_first_alive() -> Result<()> {
    let env = TestEnv::new()?;
    let mut child = spawn_daemon(&env)?;
    wait_ready(&env, Duration::from_secs(15))?;

    // A second daemon must be refused (exit non-zero, mention the holder).
    let exe = env!("CARGO_BIN_EXE_pve-mcp");
    let out = std::process::Command::new(exe)
        .arg("--config")
        .arg(&env.config_path)
        .arg("--port-dir")
        .arg(&env.port_dir)
        .arg("daemon")
        .output()
        .context("spawning second daemon")?;
    assert!(!out.status.success(), "second daemon must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("already running"),
        "refusal must name the holder: {stderr}"
    );

    // First daemon still healthy afterwards.
    let port = wait_ready(&env, Duration::from_secs(5))?;
    let status = admin_request(port, "status")?;
    assert_eq!(status["result"]["version"], "0.1.0");

    admin_request(port, "stop")?;
    wait_stopped(&env, Duration::from_secs(10))?;
    let _ = child.kill();
    Ok(())
}

#[test]
fn stale_lock_with_zombie_pid_is_taken_over() -> Result<()> {
    let env = TestEnv::new()?;
    // Spawn a child, let it EXIT, keep the handle: on Windows the pid stays
    // "openable" (zombie) while this handle lives — the naive OpenProcess
    // probe would call it alive. WaitForSingleObject(0) must see it dead.
    let mut zombie = std::process::Command::new(if cfg!(windows) { "cmd" } else { "true" })
        .args(if cfg!(windows) {
            vec!["/C", "exit"]
        } else {
            vec![]
        })
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let pid = zombie.id();
    let _ = zombie.wait();

    // Hand-write the stale lock.
    std::fs::write(env.port_dir.join("daemon.lock"), format!("{pid}\n"))?;

    // Acquire must succeed (takeover) and the daemon must start.
    let mut child = spawn_daemon(&env)?;
    let port = wait_ready(&env, Duration::from_secs(15)).context("takeover failed")?;
    let status = admin_request(port, "status")?;
    assert_eq!(status["result"]["version"], "0.1.0");
    admin_request(port, "stop")?;
    wait_stopped(&env, Duration::from_secs(10))?;
    let _ = child.kill();
    Ok(())
}

#[test]
fn live_holder_pid_lock_is_honored() -> Result<()> {
    let env = TestEnv::new()?;
    // Write a lock naming OUR OWN live pid, then spawn: the daemon must
    // refuse (holder alive).
    std::fs::write(
        env.port_dir.join("daemon.lock"),
        format!("{}\n", std::process::id()),
    )?;
    let exe = env!("CARGO_BIN_EXE_pve-mcp");
    let out = std::process::Command::new(exe)
        .arg("--config")
        .arg(&env.config_path)
        .arg("--port-dir")
        .arg(&env.port_dir)
        .arg("daemon")
        .output()
        .context("spawning daemon against live lock")?;
    assert!(
        !out.status.success(),
        "daemon must refuse a live-holder lock"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("already running"), "{stderr}");
    Ok(())
}
