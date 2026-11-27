//! Real-cluster read-only smoke: login (password path) + GET /version +
//! GET /nodes. Runs only when PVE_TEST_HOST/PVE_TEST_USER/PVE_TEST_PASSWORD
//! env vars are set (or a test config file exists); otherwise SKIP with an
//! explicit report. Read-only: GET requests only.

#![cfg(feature = "smoke")]

use pve_core::client::PveClient;
use pve_core::config::PveConfig;

fn test_config() -> Option<PveConfig> {
    let host = std::env::var("PVE_TEST_HOST").ok()?;
    let user = std::env::var("PVE_TEST_USER").ok()?;
    let password = std::env::var("PVE_TEST_PASSWORD").ok()?;
    Some(PveConfig {
        host,
        port: 8006,
        verify_ssl: false,
        ca_bundle: None,
        hostname: None,
        ssh_alias: None,
        token_id: None,
        token_value: None,
        user: Some(user),
        password: Some(password),
    })
}

#[test]
fn smoke_login_and_read_version() {
    let Some(config) = test_config() else {
        eprintln!("SKIP: PVE_TEST_HOST/PVE_TEST_USER/PVE_TEST_PASSWORD not set");
        return;
    };
    let mut client = PveClient::new(&config).expect("client builds");
    client.login().expect("password login works");
    let version: serde_json::Value = client
        .get_json("/version")
        .expect("GET /version succeeds")
        .expect("version data present");
    let release = version["version"].as_str().unwrap_or_default();
    assert!(!release.is_empty(), "version payload has release: {version}");
    eprintln!("connected: PVE {release}");
}

#[test]
fn smoke_list_nodes_read_only() {
    let Some(config) = test_config() else {
        eprintln!("SKIP: PVE_TEST_HOST/PVE_TEST_USER/PVE_TEST_PASSWORD not set");
        return;
    };
    let mut client = PveClient::new(&config).expect("client builds");
    client.login().expect("login");
    let nodes: Vec<serde_json::Value> = client
        .get_json("/nodes")
        .expect("GET /nodes succeeds")
        .unwrap_or_default();
    assert!(!nodes.is_empty(), "single-node cluster reports >= 1 node");
    for node in &nodes {
        let name = node["node"].as_str().unwrap_or_default();
        let status = node["status"].as_str().unwrap_or_default();
        eprintln!("node {name}: {status}");
        assert!(!name.is_empty());
    }
}
