//! Configuration: `pve-mcp.toml` parsing, defaults, validation.
//!
//! Lookup order: `--config` path > `PVE_MCP_CONFIG` env > `pve-mcp.toml`
//! next to the current exe > working directory. Credentials live only in
//! this file (gitignored) or environment variables.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::domain::VmidSet;
use crate::error::Result;

/// The app's per-user data directory: `%APPDATA%/pve-mcp` on Windows,
/// `$XDG_CONFIG_HOME/pve-mcp` (or `~/.config/pve-mcp`) elsewhere. Home of
/// the standard config path and the daemon port/lock files.
pub fn app_data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("PVE_MCP_HOME") {
        return Some(PathBuf::from(dir));
    }
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("pve-mcp"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|d| d.join("pve-mcp"))
    }
}

/// Config lookup order: `--config` > `PVE_MCP_CONFIG` env > app data dir
/// (`%APPDATA%/pve-mcp/pve-mcp.toml`, the deployed standard location) >
/// exe-adjacent > working directory.
pub fn default_config_path() -> Option<PathBuf> {
    if let Some(dir) = app_data_dir() {
        let candidate = dir.join("pve-mcp.toml");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("pve-mcp.toml");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    std::env::current_dir().ok().and_then(|cwd| {
        let candidate = cwd.join("pve-mcp.toml");
        candidate.exists().then_some(candidate)
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub pve: PveConfig,
    pub server: ServerConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PveConfig {
    /// API host (no scheme, no port).
    pub host: String,
    pub port: u16,
    /// Self-signed cluster cert: skip TLS verification when true.
    pub verify_ssl: bool,
    /// Trusted CA bundle path; when set, verification runs against it.
    pub ca_bundle: Option<PathBuf>,
    /// Informal hostname label (lan-pve) — informational only.
    pub hostname: Option<String>,
    /// SSH backend alias from ~/.ssh/config (root required on the PVE side).
    pub ssh_alias: Option<String>,
    /// Token auth: full id `root@pam!tokenid` and secret.
    pub token_id: Option<String>,
    pub token_value: Option<String>,
    /// Password auth: user (`root@pam`) + password → ticket (2h renew).
    pub user: Option<String>,
    pub password: Option<String>,
}

impl Default for PveConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 8006,
            verify_ssl: false,
            ca_bundle: None,
            hostname: None,
            ssh_alias: None,
            token_id: None,
            token_value: None,
            user: None,
            password: None,
        }
    }
}

impl PveConfig {
    /// A token pair is usable when both halves exist and neither is blank.
    pub fn token_pair(&self) -> Option<(&str, &str)> {
        match (self.token_id.as_deref(), self.token_value.as_deref()) {
            (Some(id), Some(value)) if !id.trim().is_empty() && !value.trim().is_empty() => {
                Some((id.trim(), value.trim()))
            }
            _ => None,
        }
    }

    pub fn password_pair(&self) -> Option<(&str, &str)> {
        match (self.user.as_deref(), self.password.as_deref()) {
            (Some(user), Some(password)) if !user.trim().is_empty() && !password.is_empty() => {
                Some((user.trim(), password))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ServerConfig {
    /// HTTP MCP transport bind address.
    pub http_bind: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            http_bind: "127.0.0.1:8766".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SecurityConfig {
    /// `read_only` shrinks the tool surface to query tools only.
    pub access_tier: AccessTier,
    /// Comma-separated vmids and ranges no tool may change, e.g. "140,141".
    pub protected_vmids: String,
    /// Default wait timeout for long tasks (seconds).
    pub wait_timeout_secs: u64,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            access_tier: AccessTier::Full,
            protected_vmids: String::new(),
            wait_timeout_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum AccessTier {
    #[default]
    Full,
    ReadOnly,
}

impl AccessTier {
    pub fn allows_writes(self) -> bool {
        match self {
            Self::Full => true,
            Self::ReadOnly => false,
        }
    }
}

impl Config {
    pub fn load(explicit: Option<&Path>) -> Result<Self> {
        let path = explicit
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("PVE_MCP_CONFIG").map(PathBuf::from))
            .or_else(default_config_path);
        let Some(path) = path else {
            return Ok(Self::default());
        };
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            crate::error::DomainError::ConfigIo(format!("reading {}: {e}", path.display()))
        })?;
        let config: Config = toml::from_str(&raw).map_err(|e| {
            crate::error::DomainError::ConfigIo(format!("parsing {}: {e}", path.display()))
        })?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if !self.pve.host.is_empty() {
            let host = &self.pve.host;
            if host.contains("://") {
                return Err(crate::error::DomainError::ConfigIo(format!(
                    "[pve].host must be bare host, not a URL: {host}"
                )));
            }
            if host.ends_with('/') {
                return Err(crate::error::DomainError::ConfigIo(format!(
                    "[pve].host must not end with '/': {host}"
                )));
            }
            if self.pve.token_pair().is_none() && self.pve.password_pair().is_none() {
                return Err(crate::error::DomainError::ConfigIo(
                    "[pve] needs either token_id+token_value or user+password".to_string(),
                ));
            }
        }
        // protected_vmids parses eagerly: garbage fails at load, not first use.
        let _ = self.protected_vmids_set()?;
        Ok(())
    }

    /// Parsed protected vmid set; empty spec = empty set.
    pub fn protected_vmids_set(&self) -> Result<VmidSet> {
        VmidSet::parse(&self.security.protected_vmids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_str(raw: &str) -> Result<Config> {
        let config: Config = toml::from_str(raw).unwrap();
        config.validate()?;
        Ok(config)
    }

    #[test]
    fn parses_minimal_token_config() {
        let config = load_str(
            r#"
[pve]
host = "192.168.88.88"
token_id = "pvebot-ro@pve!reader"
token_value = "secret"
"#,
        )
        .unwrap();
        assert_eq!(config.pve.port, 8006);
        assert!(!config.pve.verify_ssl);
        assert!(config.pve.token_pair().is_some());
        assert!(config.security.access_tier.allows_writes());
    }

    #[test]
    fn parses_password_config_with_defaults() {
        let config = load_str(
            r#"
[pve]
host = "192.168.88.88"
user = "root@pam"
password = "test-password-with-!@-specials"
"#,
        )
        .unwrap();
        assert!(config.pve.password_pair().is_some());
        assert_eq!(config.server.http_bind, "127.0.0.1:8766");
        assert_eq!(config.security.wait_timeout_secs, 300);
    }

    #[test]
    fn rejects_url_host() {
        assert!(load_str(
            r#"
[pve]
host = "https://192.168.88.88:8006"
token_id = "a"
token_value = "b"
"#
        )
        .is_err());
    }

    #[test]
    fn rejects_host_without_credentials() {
        assert!(load_str(
            r#"
[pve]
host = "192.168.88.88"
"#
        )
        .is_err());
    }

    #[test]
    fn empty_config_is_default() {
        let config = load_str("").unwrap();
        assert!(config.pve.token_pair().is_none());
        assert!(config.pve.host.is_empty());
    }

    #[test]
    fn deny_unknown_fields() {
        let result: std::result::Result<Config, _> = toml::from_str(
            r#"
[pve]
hostt = "typo"
"#,
        );
        assert!(result.is_err());
    }

    #[test]
    fn protected_vmids_parse_at_load() {
        let config = load_str(
            r#"
[pve]
host = "h"
token_id = "a"
token_value = "b"

[security]
protected_vmids = "140,141,106"
"#,
        )
        .unwrap();
        let set = config.protected_vmids_set().unwrap();
        assert!(set.contains(crate::domain::Vmid::try_from(140u32).unwrap()));
        assert!(!set.contains(crate::domain::Vmid::try_from(102u32).unwrap()));
    }

    #[test]
    fn protected_vmids_garbage_fails_load() {
        assert!(load_str(
            r#"
[pve]
host = "h"
token_id = "a"
token_value = "b"

[security]
protected_vmids = "not,vmids"
"#
        )
        .is_err());
    }

    #[test]
    fn access_tier_read_only() {
        let config: Config = toml::from_str(
            r#"
[pve]
host = "h"
token_id = "a"
token_value = "b"

[security]
access_tier = "read_only"
"#,
        )
        .unwrap();
        assert!(!config.security.access_tier.allows_writes());
    }

    #[test]
    fn token_pair_trims_and_rejects_blank() {
        let mut config = Config::default();
        config.pve.token_id = Some("  root@pam!t  ".into());
        config.pve.token_value = Some(" s ".into());
        assert_eq!(config.pve.token_pair(), Some(("root@pam!t", "s")));
        config.pve.token_value = Some("   ".into());
        assert!(config.pve.token_pair().is_none());
    }
}
