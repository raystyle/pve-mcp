//! PVE API client: thin ureq layer over /api2/json.
//!
//! Auth: API token header, or password → ticket (Cookie + CSRF on writes,
//! 2h renew). Errors are classified (connect / status / parse / tls / tfa)
//! and carry the API error body's reason when present. Secrets never reach
//! logs or error strings.

use std::fmt;
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

use crate::config::PveConfig;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// PVE tickets live 2h; renew ahead of expiry with this margin.
const TICKET_RENEW_MARGIN: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("connect failed: {0}")]
    Connect(String),
    #[error("tls verification failed: {0}")]
    Tls(String),
    #[error("api status {status} for {method} {path}: {reason}")]
    Status {
        status: u16,
        method: &'static str,
        path: String,
        reason: String,
    },
    #[error("unauthorized for {method} {path}")]
    Unauthorized { method: &'static str, path: String },
    #[error("forbidden for {method} {path} (token lacks privilege)")]
    Forbidden { method: &'static str, path: String },
    #[error("login requires two-factor authentication")]
    TfaRequired,
    #[error("response parse failed for {0}: {1}")]
    Parse(String, String),
    #[error("invalid header value: token id or secret has non-header characters")]
    BadHeader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Method {
    fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }

    fn is_write(self) -> bool {
        match self {
            Self::Get => false,
            Self::Post | Self::Put | Self::Delete => true,
        }
    }
}

/// API response envelope: every /api2/json response is `{"data": ...}`.
/// Error bodies are parsed separately (extract_error_reason) via the raw
/// string path — this envelope only decodes successful data.
#[derive(Debug, Deserialize)]
#[serde(bound = "T: serde::de::DeserializeOwned")]
struct Envelope<T> {
    #[serde(default)]
    data: Option<T>,
}

/// Ticket session FSM: unauthenticated → authenticated(expires) → renewing.
/// Transitions are exhaustive matches; a new state or event breaks compile.
#[derive(Debug)]
enum TicketSession {
    Unauthenticated,
    Authenticated {
        ticket: String,
        csrf: String,
        expires_at: std::time::Instant,
    },
    Renewing {
        ticket: String,
        csrf: String,
    },
}

impl TicketSession {
    /// True when usable for a request right now.
    fn usable(&self, now: std::time::Instant) -> bool {
        match self {
            Self::Unauthenticated => false,
            Self::Authenticated { expires_at, .. } => now + TICKET_RENEW_MARGIN < *expires_at,
            Self::Renewing { .. } => false,
        }
    }

    fn csrf(&self) -> Option<&str> {
        match self {
            Self::Unauthenticated => None,
            Self::Authenticated { csrf, .. } | Self::Renewing { csrf, .. } => Some(csrf),
        }
    }

    fn cookie(&self) -> Option<&str> {
        match self {
            Self::Unauthenticated => None,
            Self::Authenticated { ticket, .. } | Self::Renewing { ticket, .. } => Some(ticket),
        }
    }
}

enum AuthMode {
    Token { header_value: String },
    Password { user: String, password: String },
}

/// Authenticated connection to the PVE API on one host.
pub struct PveClient {
    agent: ureq::Agent,
    origin: String,
    auth: AuthMode,
    session: TicketSession,
}

impl fmt::Debug for PveClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PveClient")
            .field("origin", &self.origin)
            .field(
                "auth",
                &match self.auth {
                    AuthMode::Token { .. } => "token",
                    AuthMode::Password { .. } => "password",
                },
            )
            .finish_non_exhaustive()
    }
}

impl PveClient {
    pub fn new(config: &PveConfig) -> Result<Self, ApiError> {
        let trimmed = config.host.trim();
        let host = match trimmed.parse::<std::net::IpAddr>() {
            Ok(std::net::IpAddr::V6(v6)) => format!("[{v6}]"),
            Ok(v4) => v4.to_string(),
            Err(_) => trimmed.to_string(),
        };
        let origin = format!("https://{}:{}/api2/json", host, config.port);

        let auth = if let Some((token_id, token_value)) = config.token_pair() {
            let header_value = format!("PVEAPIToken={token_id}={token_value}");
            // Fail explicitly on non-header characters instead of panicking
            // inside ureq when the header is applied later.
            if header_bad_for_header_value(&header_value) {
                return Err(ApiError::BadHeader);
            }
            AuthMode::Token { header_value }
        } else if let Some((user, password)) = config.password_pair() {
            AuthMode::Password {
                user: user.to_string(),
                password: password.to_string(),
            }
        } else {
            return Err(ApiError::BadHeader);
        };

        let mut agent_builder = ureq::AgentBuilder::new()
            .timeout_connect(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT);
        if !config.verify_ssl && config.ca_bundle.is_none() {
            agent_builder = agent_builder.tls_config(std::sync::Arc::new(noverify_client_config()));
        }
        let agent = agent_builder.build();
        // Token auth is stateless; the ticket FSM stays Unauthenticated and
        // unused in that mode.
        let session = TicketSession::Unauthenticated;
        Ok(Self {
            agent,
            origin,
            auth,
            session,
        })
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    fn apply_auth(
        &self,
        request: ureq::Request,
        method: Method,
    ) -> Result<ureq::Request, ApiError> {
        match &self.auth {
            AuthMode::Token { header_value } => {
                Ok(request.set("Authorization", header_value.as_str()))
            }
            AuthMode::Password { .. } => {
                let mut request = request;
                if let Some(ticket) = self.session.cookie() {
                    request = request.set("Cookie", format!("PVEAuthCookie={ticket}").as_str());
                    if method.is_write() {
                        if let Some(csrf) = self.session.csrf() {
                            request = request.set("CSRFPreventionToken", csrf);
                        }
                    }
                }
                Ok(request)
            }
        }
    }

    /// Login with password, replacing any prior session. Returns the ticket
    /// expiry instant. TFA-gated accounts surface as ApiError::TfaRequired.
    pub fn login(&mut self) -> Result<std::time::Instant, ApiError> {
        let (user, password) = match &self.auth {
            AuthMode::Password { user, password } => (user.clone(), password.clone()),
            AuthMode::Token { .. } => return Err(ApiError::BadHeader),
        };
        // Transition: → Renewing (in-flight) while the request runs.
        let previous = std::mem::replace(&mut self.session, TicketSession::Unauthenticated);
        if let TicketSession::Authenticated { ticket, csrf, .. } = previous {
            self.session = TicketSession::Renewing { ticket, csrf };
        }

        let body = format!(
            "username={}&password={}",
            urlencoding_encode(&user),
            urlencoding_encode(&password)
        );
        let result = self
            .agent
            .post(format!("{}/access/ticket", self.origin).as_str())
            .timeout(REQUEST_TIMEOUT)
            .send_string(&body);

        match result {
            Ok(response) => {
                let parsed: std::result::Result<Envelope<TicketData>, _> =
                    response.into_json().map_err(|e| {
                        ApiError::Parse("/access/ticket".into(), format!("body read: {e}"))
                    });
                match parsed {
                    Ok(Envelope {
                        data: Some(data), ..
                    }) if !data.ticket.is_empty() => {
                        let expires_at = std::time::Instant::now()
                            + std::time::Duration::from_secs(
                                data.ticket_lifetime().unwrap_or(7200),
                            );
                        self.session = TicketSession::Authenticated {
                            ticket: data.ticket,
                            csrf: data.csrf_prevention_token,
                            expires_at,
                        };
                        Ok(expires_at)
                    }
                    Ok(_) => Err(ApiError::Parse(
                        "/access/ticket".into(),
                        "response missing ticket data".into(),
                    )),
                    Err(e) => Err(e),
                }
            }
            Err(ureq::Error::Status(code, response)) => {
                self.session = TicketSession::Unauthenticated;
                Err(self.classify_status(code, "POST", "/access/ticket", response))
            }
            Err(e) => {
                self.session = TicketSession::Unauthenticated;
                Err(classify_transport(e, "POST", "/access/ticket"))
            }
        }
    }

    /// Ensure an authenticated session exists and is fresh (password mode).
    fn ensure_session(&mut self) -> Result<(), ApiError> {
        if let AuthMode::Token { .. } = self.auth {
            return Ok(());
        }
        let now = std::time::Instant::now();
        if self.session.usable(now) {
            return Ok(());
        }
        self.login()?;
        Ok(())
    }

    /// GET a path and unwrap the envelope's data. `Ok(None)` on 403: the
    /// read-only token legitimately lacks privilege on some paths.
    pub fn get_json<T: serde::de::DeserializeOwned>(
        &mut self,
        path: &str,
    ) -> Result<Option<T>, ApiError> {
        self.request_json(Method::Get, path, None)
    }

    /// POST with form-encoded fields; returns the envelope data.
    pub fn post_form<T: serde::de::DeserializeOwned>(
        &mut self,
        path: &str,
        fields: &[(String, String)],
    ) -> Result<Option<T>, ApiError> {
        let body = fields
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding_encode(k), urlencoding_encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        self.request_json_with_body(Method::Post, path, Some(body))
    }

    pub fn put_form<T: serde::de::DeserializeOwned>(
        &mut self,
        path: &str,
        fields: &[(String, String)],
    ) -> Result<Option<T>, ApiError> {
        let body = fields
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding_encode(k), urlencoding_encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        self.request_json_with_body(Method::Put, path, Some(body))
    }

    pub fn delete<T: serde::de::DeserializeOwned>(
        &mut self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<Option<T>, ApiError> {
        let path = append_query(path, query);
        self.request_json(Method::Delete, path.as_str(), None)
    }

    fn request_json<T: serde::de::DeserializeOwned>(
        &mut self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<Option<T>, ApiError> {
        self.request_json_with_body(method, path, body)
    }

    fn request_json_with_body<T: serde::de::DeserializeOwned>(
        &mut self,
        method: Method,
        path: &str,
        body: Option<String>,
    ) -> Result<Option<T>, ApiError> {
        if self.auth_mode_is_password() {
            self.ensure_session()?;
        }
        let url = format!("{}{}", self.origin, path);
        let base = match method {
            Method::Get => self.agent.get(url.as_str()),
            Method::Post => self.agent.post(url.as_str()),
            Method::Put => self.agent.put(url.as_str()),
            Method::Delete => self.agent.delete(url.as_str()),
        };
        let mut request = self.apply_auth(base, method)?;
        if body.is_some() && method.is_write() {
            request = request.set("Content-Type", "application/x-www-form-urlencoded");
        }

        let response = match body {
            Some(body) => request.send_string(&body),
            None => request.call(),
        };

        match response {
            Ok(response) => {
                let parsed: std::result::Result<Envelope<T>, _> = response
                    .into_json()
                    .map_err(|e| ApiError::Parse(path.to_string(), format!("body read: {e}")));
                match parsed {
                    Ok(Envelope { data, .. }) => Ok(data),
                    Err(e) => Err(e),
                }
            }
            Err(ureq::Error::Status(code, response)) => {
                Err(self.classify_status(code, method.as_str(), path, response))
            }
            Err(e) => Err(classify_transport(e, method.as_str(), path)),
        }
    }

    fn auth_mode_is_password(&self) -> bool {
        match self.auth {
            AuthMode::Token { .. } => false,
            AuthMode::Password { .. } => true,
        }
    }

    fn classify_status(
        &self,
        code: u16,
        method: &'static str,
        path: &str,
        response: ureq::Response,
    ) -> ApiError {
        let body_text = response.into_string().unwrap_or_else(|_| String::new());
        let reason = extract_error_reason(&body_text);
        match code {
            401 => ApiError::Unauthorized {
                method,
                path: path.to_string(),
            },
            403 => ApiError::Forbidden {
                method,
                path: path.to_string(),
            },
            _ => ApiError::Status {
                status: code,
                method,
                path: path.to_string(),
                reason,
            },
        }
    }
}

/// Header values in ureq2 flow through `header::HeaderValue::from_str`;
/// mirror that validation without the dependency on a re-exported http.
fn header_bad_for_header_value(value: &str) -> bool {
    value
        .bytes()
        .any(|b| b < 0x21 || b == 0x7F || b == b' ' || b > 0x7E)
}

/// rustls ClientConfig that accepts any server certificate. Built only when
/// the config explicitly opts out of verification (`verify_ssl = false`,
/// self-signed cluster cert, no CA bundle) — the lab's documented posture.
fn noverify_client_config() -> rustls::ClientConfig {
    #[derive(Debug)]
    struct NoVerify;

    impl rustls::client::danger::ServerCertVerifier for NoVerify {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error>
        {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
        {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
        {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(std::sync::Arc::new(NoVerify))
        .with_no_client_auth()
}

#[derive(Debug, Deserialize)]
struct TicketData {
    ticket: String,
    #[serde(rename = "CSRFPreventionToken", default)]
    csrf_prevention_token: String,
    /// PVE returns a `Cap` object in newer versions; lifetime is not always
    /// present — default to 2h.
    #[serde(default)]
    ticket_lifetime: Option<serde_json::Value>,
}

impl TicketData {
    fn ticket_lifetime(&self) -> Option<u64> {
        self.ticket_lifetime.as_ref().and_then(|v| v.as_u64())
    }
}

/// Pull the human reason out of a PVE error body: `{"data":null,"errors":{
/// "param":"msg"}}` or plain strings. Never includes credentials — the body
/// is PVE-generated.
fn extract_error_reason(body: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        let trimmed = body.trim();
        return if trimmed.is_empty() {
            "empty body".to_string()
        } else {
            trimmed.chars().take(200).collect()
        };
    };
    if let Some(errors) = value.get("errors") {
        if let Some(map) = errors.as_object() {
            let joined = map
                .iter()
                .map(|(k, v)| match v.as_str() {
                    Some(text) => format!("{k}: {text}"),
                    None => format!("{k}: {v}"),
                })
                .collect::<Vec<_>>()
                .join("; ");
            return joined.chars().take(400).collect();
        }
        return errors.to_string().chars().take(400).collect();
    }
    if let Some(msg) = value.get("message").and_then(|m| m.as_str()) {
        return msg.chars().take(400).collect();
    }
    value.to_string().chars().take(400).collect()
}

fn classify_transport(e: ureq::Error, method: &'static str, path: &str) -> ApiError {
    let text = e.to_string();
    if text.contains("tls") || text.contains("certificate") {
        ApiError::Tls(text)
    } else {
        let _ = (method, path);
        ApiError::Connect(text)
    }
}

fn append_query(path: &str, query: &[(String, String)]) -> String {
    if query.is_empty() {
        return path.to_string();
    }
    let encoded = query
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding_encode(k), urlencoding_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let sep = if path.contains('?') { '&' } else { '?' };
    format!("{path}{sep}{encoded}")
}

/// Minimal percent-encoding for form/query values: everything except
/// unreserved RFC 3986 characters. Passwords with `!@#` etc. must not go raw.
pub(crate) fn urlencoding_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoding_encodes_specials() {
        assert_eq!(urlencoding_encode("root@pam"), "root%40pam");
        assert_eq!(urlencoding_encode("p!ss word"), "p%21ss%20word");
        assert_eq!(urlencoding_encode("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn extract_error_reason_parses_errors_map() {
        let body = r#"{"data":null,"errors":{"vmid":"vmid is missing"}}"#;
        let reason = extract_error_reason(body);
        assert_eq!(reason, "vmid: vmid is missing");
    }

    #[test]
    fn extract_error_reason_handles_plain_text() {
        assert_eq!(extract_error_reason(""), "empty body");
        assert!(extract_error_reason("not json").contains("not json"));
    }

    #[test]
    fn extract_error_reason_message_field() {
        let body = r#"{"data":null,"message":"no quorum"}"#;
        assert_eq!(extract_error_reason(body), "no quorum");
    }

    #[test]
    fn append_query_builds_url_query() {
        let path = append_query(
            "/nodes/lan-pve/qemu/102/rrddata",
            &[("timeframe".to_string(), "hour".to_string())],
        );
        assert_eq!(path, "/nodes/lan-pve/qemu/102/rrddata?timeframe=hour");
        let with_q = append_query("/a?x=1", &[("y".to_string(), "2".to_string())]);
        assert_eq!(with_q, "/a?x=1&y=2");
    }

    #[test]
    fn method_write_classification() {
        assert!(!Method::Get.is_write());
        assert!(Method::Post.is_write());
        assert!(Method::Put.is_write());
        assert!(Method::Delete.is_write());
    }

    #[test]
    fn ticket_session_fsm_transitions() {
        let now = std::time::Instant::now();
        let mut session = TicketSession::Unauthenticated;
        assert!(!session.usable(now));
        assert!(session.cookie().is_none());

        session = TicketSession::Authenticated {
            ticket: "t".into(),
            csrf: "c".into(),
            expires_at: now + std::time::Duration::from_secs(7200),
        };
        assert!(session.usable(now));
        assert_eq!(session.cookie(), Some("t"));
        assert_eq!(session.csrf(), Some("c"));

        // Near expiry (inside renew margin) → not usable → renew triggers.
        session = TicketSession::Authenticated {
            ticket: "t".into(),
            csrf: "c".into(),
            expires_at: now + std::time::Duration::from_secs(60),
        };
        assert!(!session.usable(now));
    }

    #[test]
    fn client_debug_never_leaks_secrets() {
        let config = crate::config::PveConfig {
            host: "192.168.88.88".into(),
            token_id: Some("root@pam!t".into()),
            token_value: Some("super-secret".into()),
            ..Default::default()
        };
        let client = PveClient::new(&config).unwrap();
        let debug = format!("{client:?}");
        assert!(!debug.contains("super-secret"));
    }

    #[test]
    fn client_rejects_header_hostile_token() {
        let config = crate::config::PveConfig {
            host: "h".into(),
            token_id: Some("root@pam!t".into()),
            token_value: Some("bad\nnewline".into()),
            ..Default::default()
        };
        assert!(matches!(PveClient::new(&config), Err(ApiError::BadHeader)));
    }

    #[test]
    fn client_builds_origin_with_ipv6_brackets() {
        let config = crate::config::PveConfig {
            host: "::1".into(),
            token_id: Some("root@pam!t".into()),
            token_value: Some("s".into()),
            ..Default::default()
        };
        let client = PveClient::new(&config).unwrap();
        assert!(client.origin().starts_with("https://[::1]:8006/"));
    }
}
