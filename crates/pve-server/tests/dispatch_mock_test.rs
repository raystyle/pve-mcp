//! Dispatch-layer integration coverage: a mock PVE API server (axum)
//! stands in for the cluster so admin-plane arms exercise their real
//! request path — URL shape, envelope unwrap, guard math — without a
//! live cluster.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::{json, Value};

#[derive(Default)]
struct MockState {
    guests: Value,
    current_status: Value,
    config: Value,
    version: Value,
    rrddata: Value,
    agent_osinfo: Value,
    nextid: Value,
    snapshot_list: Value,
}

fn mock_router(state: Arc<std::sync::Mutex<MockState>>) -> axum::Router {
    use axum::extract::{Path, State};
    use axum::http::StatusCode;
    use axum::routing::get;

    async fn version(State(st): State<Arc<std::sync::Mutex<MockState>>>) -> axum::Json<Value> {
        let v = st.lock().unwrap().version.clone();
        axum::Json(json!({ "data": v }))
    }
    async fn nodes_list() -> axum::Json<Value> {
        axum::Json(json!({ "data": [
            {"node": "lan-pve", "status": "online", "cpu": 0.03, "maxcpu": 32, "mem": 42949672960u64, "maxmem": 98784247808u64, "uptime": 100}
        ] }))
    }
    async fn resources(State(st): State<Arc<std::sync::Mutex<MockState>>>) -> axum::Json<Value> {
        let v = st.lock().unwrap().guests.clone();
        axum::Json(json!({ "data": v }))
    }
    async fn guest_current(
        State(st): State<Arc<std::sync::Mutex<MockState>>>,
        Path((node, ty, vmid)): Path<(String, String, u32)>,
    ) -> axum::Json<Value> {
        let st = st.lock().unwrap();
        let mut v = st.current_status.clone();
        if let Some(obj) = v.as_object_mut() {
            obj.insert("node".into(), json!(node));
            obj.insert("vmid".into(), json!(vmid));
            obj.insert("_ty".into(), json!(ty));
        }
        axum::Json(json!({ "data": v }))
    }
    async fn guest_config(State(st): State<Arc<std::sync::Mutex<MockState>>>) -> axum::Json<Value> {
        let v = st.lock().unwrap().config.clone();
        axum::Json(json!({ "data": v }))
    }
    async fn rrddata(State(st): State<Arc<std::sync::Mutex<MockState>>>) -> axum::Json<Value> {
        let v = st.lock().unwrap().rrddata.clone();
        axum::Json(json!({ "data": v }))
    }
    async fn agent_osinfo(State(st): State<Arc<std::sync::Mutex<MockState>>>) -> axum::Json<Value> {
        let v = st.lock().unwrap().agent_osinfo.clone();
        axum::Json(json!({ "data": v }))
    }
    async fn nextid(State(st): State<Arc<std::sync::Mutex<MockState>>>) -> axum::Json<Value> {
        let v = st.lock().unwrap().nextid.clone();
        axum::Json(json!({ "data": v }))
    }
    async fn snapshots(State(st): State<Arc<std::sync::Mutex<MockState>>>) -> axum::Json<Value> {
        let v = st.lock().unwrap().snapshot_list.clone();
        axum::Json(json!({ "data": v }))
    }
    /// A write endpoint that always 403s: proves the Forbidden path flows
    /// through the dispatch as an error string.
    async fn forbidden_write() -> (StatusCode, axum::Json<Value>) {
        (
            StatusCode::FORBIDDEN,
            axum::Json(json!({"errors": {"perm": "not allowed"}})),
        )
    }

    axum::Router::new()
        .route("/api2/json/version", get(version))
        .route("/api2/json/nodes", get(nodes_list))
        .route("/api2/json/cluster/resources", get(resources))
        .route(
            "/api2/json/nodes/{node}/{ty}/{vmid}/status/current",
            get(guest_current),
        )
        .route(
            "/api2/json/nodes/{node}/{ty}/{vmid}/config",
            get(guest_config),
        )
        .route("/api2/json/nodes/{node}/{ty}/{vmid}/rrddata", get(rrddata))
        .route(
            "/api2/json/nodes/{node}/{ty}/{vmid}/agent/get-osinfo",
            get(agent_osinfo),
        )
        .route("/api2/json/cluster/nextid", get(nextid))
        .route(
            "/api2/json/nodes/{node}/{ty}/{vmid}/snapshot",
            get(snapshots),
        )
        .route(
            "/api2/json/nodes/{node}/{ty}/{vmid}/status/start",
            axum::routing::post(forbidden_write),
        )
        .with_state(state)
}

fn spawn_mock_pve() -> Result<(SocketAddr, std::thread::JoinHandle<()>)> {
    // rustls 0.23 needs a CryptoProvider selected when multiple feature
    // sets are in play; ring is what the workspace pins.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let state = Arc::new(std::sync::Mutex::new(MockState {
        guests: json!([
            {"vmid": 102, "name": "win-target-01", "type": "qemu", "status": "running", "node": "lan-pve", "maxmem": 4294967296u64, "maxdisk": 85899345920u64, "maxcpu": 2.0, "template": 0},
            {"vmid": 140, "name": "dmz1-bridge", "type": "lxc", "status": "running", "node": "lan-pve", "maxmem": 536870912u64, "maxdisk": 4294967296u64, "maxcpu": 2.0, "template": 0}
        ]),
        current_status: json!({"status": "running", "uptime": 1000}),
        config: json!({"cores": "2", "memory": "4096", "name": "win-target-01"}),
        version: json!({"version": "9.2.21", "release": "9.2"}),
        rrddata: json!([{"cpu": 0.1, "time": 1}, {"cpu": 0.2, "time": 2}]),
        agent_osinfo: json!({"name": "Microsoft Windows"}),
        nextid: json!("103"),
        snapshot_list: json!([
            {"name": "current"},
            {"name": "pre-clone", "description": "test", "snaptime": 1700000000i64, "vmstate": null},
        ]),
    }));
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            // The product client is https-only; the mock must speak TLS.
            // Self-signed cert + accept-all client trust are both fine:
            // the daemon config runs verify_ssl = false.
            let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()])
                .context("mock cert")?;
            let tls_cfg = rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(
                    vec![rustls::pki_types::CertificateDer::from(
                        cert.cert.der().to_vec(),
                    )],
                    rustls::pki_types::PrivateKeyDer::try_from(cert.key_pair.serialize_der())
                        .map_err(|e| anyhow::anyhow!("mock key: {e}"))?,
                )
                .context("mock tls config")?;
            let tls = tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(tls_cfg));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .context("mock bind")?;
            let addr = listener.local_addr().context("mock local addr")?;
            tx.send(addr).unwrap();
            let state = state;
            loop {
                let (socket, _) = listener.accept().await?;
                let tls = tls.clone();
                let router = mock_router(state.clone());
                tokio::spawn(async move {
                    let Ok(stream) = tls.accept(socket).await else {
                        return;
                    };
                    // Per-connection HTTP/1.1 over TLS via hyper-util's
                    // auto Builder (axum::serve wants the Listener trait,
                    // which a TlsStream does not satisfy).
                    use hyper_util::rt::{TokioExecutor, TokioIo};
                    let executor = TokioExecutor::new();
                    let hyper_svc = hyper_util::service::TowerToHyperService::new(router);
                    let builder = hyper_util::server::conn::auto::Builder::new(&executor);
                    let conn = builder.serve_connection(TokioIo::new(stream), hyper_svc);
                    let _ = std::pin::pin!(conn).await;
                });
            }
            #[allow(unreachable_code)]
            {
                Ok::<(), anyhow::Error>(())
            }
        })
        .unwrap();
    });
    let addr = rx
        .recv_timeout(Duration::from_secs(5))
        .context("mock server did not bind in time")?;
    Ok((addr, handle))
}

const TEST_CONFIG_BASE: &str = r#"
[pve]
host = "127.0.0.1"
port = {PORT}
verify_ssl = false
token_id = "pvebot-ro@pve!mcp"
token_value = "mock-secret"

[security]
protected_vmids = "140-141"
"#;

struct MockEnv {
    _dir: tempfile::TempDir,
    port_dir: PathBuf,
    config_path: PathBuf,
}

fn setup_env(mock_port: u16) -> Result<MockEnv> {
    let dir = tempfile::tempdir()?;
    let port_dir = dir.path().join("portdir");
    std::fs::create_dir_all(&port_dir)?;
    let config_path = dir.path().join("pve-mcp.toml");
    let config = TEST_CONFIG_BASE.replace("{PORT}", &mock_port.to_string());
    std::fs::write(&config_path, config)?;
    Ok(MockEnv {
        _dir: dir,
        port_dir,
        config_path,
    })
}

fn spawn_daemon(env: &MockEnv) -> Result<std::process::Child> {
    let exe = env!("CARGO_BIN_EXE_pve-mcp");
    std::process::Command::new(exe)
        .arg("--config")
        .arg(&env.config_path)
        .arg("--port-dir")
        .arg(&env.port_dir)
        .arg("daemon")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawning daemon against mock")
}

fn wait_ready(env: &MockEnv, timeout: Duration) -> Result<u16> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if std::time::Instant::now() > deadline {
            anyhow::bail!("daemon did not become ready in time");
        }
        let port_file = env.port_dir.join("admin.port");
        if let Ok(text) = std::fs::read_to_string(&port_file) {
            if let Ok(port) = text.trim().parse::<u16>() {
                if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                    return Ok(port);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn admin(port: u16, method: &str, params: Value) -> Result<Value> {
    use std::io::{BufRead, BufReader, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let request = json!({"id": 1, "method": method, "params": params});
    writeln!(stream, "{request}")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    anyhow::ensure!(!line.is_empty(), "no reply");
    Ok(serde_json::from_str(line.trim())?)
}

fn stop_daemon(port: u16) {
    let _ = admin(port, "stop", json!({}));
    std::thread::sleep(Duration::from_millis(300));
}

#[test]
fn cluster_status_arm_returns_version_and_counts() -> Result<()> {
    let (addr, mock) = spawn_mock_pve()?;
    let env = setup_env(addr.port())?;
    let mut child = spawn_daemon(&env)?;
    let port = match wait_ready(&env, Duration::from_secs(15)) {
        Ok(p) => p,
        Err(e) => {
            if let Ok(dead) = child.wait() {
                let mut err = String::new();
                use std::io::Read;
                if let Some(mut se) = child.stderr.take() {
                    let _ = se.read_to_string(&mut err);
                }
                anyhow::bail!("daemon died: {e}; exit={dead:?}; stderr={err}");
            }
            return Err(e);
        }
    };

    let reply = admin(port, "cluster_status", json!({}))?;
    let result = &reply["result"];
    assert_eq!(result["pve_version"], "9.2.21");
    assert_eq!(result["guests"]["total"], 2);
    assert_eq!(result["guests"]["lxc"], 1);

    stop_daemon(port);
    let _ = child.kill();
    let _ = mock;
    Ok(())
}

#[test]
fn vm_list_arm_filters_by_type() -> Result<()> {
    let (addr, mock) = spawn_mock_pve()?;
    let env = setup_env(addr.port())?;
    let mut child = spawn_daemon(&env)?;
    let port = wait_ready(&env, Duration::from_secs(15))?;

    let reply = admin(port, "vm_list", json!({"ty": "lxc"}))?;
    let result = &reply["result"];
    assert_eq!(result["count"], 1);
    assert_eq!(result["guests"][0]["vmid"], 140);

    stop_daemon(port);
    let _ = child.kill();
    let _ = mock;
    Ok(())
}

#[test]
fn vm_status_arm_merges_current_and_config() -> Result<()> {
    let (addr, mock) = spawn_mock_pve()?;
    let env = setup_env(addr.port())?;
    let mut child = spawn_daemon(&env)?;
    let port = wait_ready(&env, Duration::from_secs(15))?;

    let reply = admin(port, "vm_status", json!({"vmid": 102}))?;
    let result = &reply["result"];
    assert_eq!(result["node"], "lan-pve");
    assert_eq!(result["current"]["status"], "running");
    assert_eq!(result["config"]["cores"], "2");

    stop_daemon(port);
    let _ = child.kill();
    let _ = mock;
    Ok(())
}

#[test]
fn vm_metrics_arm_downsamples_rrddata() -> Result<()> {
    let (addr, mock) = spawn_mock_pve()?;
    let env = setup_env(addr.port())?;
    let mut child = spawn_daemon(&env)?;
    let port = wait_ready(&env, Duration::from_secs(15))?;

    let reply = admin(
        port,
        "vm_metrics",
        json!({"vmid": 102, "timeframe": "hour"}),
    )?;
    let result = &reply["result"];
    assert_eq!(result["points"], 2);
    assert_eq!(result["data"][0]["cpu"], 0.1);

    let bad = admin(
        port,
        "vm_metrics",
        json!({"vmid": 102, "timeframe": "decade"}),
    )?;
    assert!(bad["error"].as_str().unwrap().contains("hour|day"));

    stop_daemon(port);
    let _ = child.kill();
    let _ = mock;
    Ok(())
}

#[test]
fn vm_agent_arm_qemu_only() -> Result<()> {
    let (addr, mock) = spawn_mock_pve()?;
    let env = setup_env(addr.port())?;
    let mut child = spawn_daemon(&env)?;
    let port = wait_ready(&env, Duration::from_secs(15))?;

    let reply = admin(port, "vm_agent", json!({"vmid": 102, "what": "osinfo"}))?;
    assert_eq!(reply["result"]["osinfo"]["name"], "Microsoft Windows");

    let lxc = admin(port, "vm_agent", json!({"vmid": 140, "what": "ping"}))?;
    assert!(lxc["error"].as_str().unwrap().contains("qemu-only"));

    stop_daemon(port);
    let _ = child.kill();
    let _ = mock;
    Ok(())
}

#[test]
fn snapshot_manage_list_filters_current() -> Result<()> {
    let (addr, mock) = spawn_mock_pve()?;
    let env = setup_env(addr.port())?;
    let mut child = spawn_daemon(&env)?;
    let port = wait_ready(&env, Duration::from_secs(15))?;

    let reply = admin(
        port,
        "snapshot_manage",
        json!({"vmid": 102, "action": "list"}),
    )?;
    let result = &reply["result"];
    assert_eq!(result["count"], 1); // "current" filtered
    assert_eq!(result["snapshots"][0]["name"], "pre-clone");

    stop_daemon(port);
    let _ = child.kill();
    let _ = mock;
    Ok(())
}

#[test]
fn protected_vmid_blocks_power_before_any_api_write() -> Result<()> {
    let (addr, mock) = spawn_mock_pve()?;
    let env = setup_env(addr.port())?;
    let mut child = spawn_daemon(&env)?;
    let port = wait_ready(&env, Duration::from_secs(15))?;

    let reply = admin(
        port,
        "vm_power",
        json!({"vmid": 140, "action": "shutdown", "confirm": true}),
    )?;
    assert!(
        reply["error"].as_str().unwrap().contains("protected"),
        "guard must refuse: {}",
        reply["error"]
    );

    stop_daemon(port);
    let _ = child.kill();
    let _ = mock;
    Ok(())
}

#[test]
fn read_only_tier_disables_write_arms() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let port_dir = dir.path().join("portdir");
    std::fs::create_dir_all(&port_dir)?;
    let (addr, _mock) = spawn_mock_pve()?;
    let config_path = dir.path().join("pve-mcp.toml");
    // Single [security] section: extend the base (double section = TOML
    // parse error, daemon dies at load, wait_ready times out).
    std::fs::write(
        &config_path,
        format!(
            "{}\naccess_tier = \"read_only\"\n",
            TEST_CONFIG_BASE
                .replace("{PORT}", &addr.port().to_string())
                .trim_end()
        ),
    )?;
    let env = MockEnv {
        _dir: dir,
        port_dir,
        config_path,
    };
    let mut child = spawn_daemon(&env)?;
    let port = wait_ready(&env, Duration::from_secs(15))?;

    let reply = admin(
        port,
        "vm_power",
        json!({"vmid": 102, "action": "start", "confirm": true}),
    )?;
    assert!(
        reply["error"].as_str().unwrap().contains("read_only"),
        "tier must gate writes: {}",
        reply["error"]
    );

    stop_daemon(port);
    let _ = child.kill();
    Ok(())
}
