//! Daemon: the long-lived background process owning the PVE client and the
//! admin plane. herdr pattern — probe-then-spawn-then-wait for clients.
//!
//! Lifecycle is a typestate chain: `Unlocked → Locked → Bound → Ready`.
//! "Acquire the lock BEFORE binding the port" is enforced by the type
//! system — `bind()` exists only on `Daemon<Locked>`, and the port file is
//! written only after a successful bind.
//!
//! The admin plane is JSON-per-line over TCP loopback; the port lands in
//! `admin.port` via temp-file + atomic rename.

use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use pve_core::client::PveClient;
use pve_core::config::Config;

const READY_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

// ---------------------------------------------------------------------------
// Typestate markers
// ---------------------------------------------------------------------------

pub struct Unlocked;
pub struct Locked;
pub struct Bound;
pub struct Ready;

pub struct Daemon<State> {
    port_dir: PathBuf,
    config: Config,
    state: std::marker::PhantomData<State>,
}

impl Daemon<Unlocked> {
    pub fn new(port_dir: PathBuf, config: Config) -> Self {
        Self {
            port_dir,
            config,
            state: std::marker::PhantomData,
        }
    }

    /// Acquire the single-instance lock. The lock is the FIRST artifact in
    /// the port dir (parents created first — create_new does not create
    /// them). A live holder aborts; a stale lock (dead PID) is taken over.
    pub fn acquire_lock(self) -> Result<Daemon<Locked>> {
        std::fs::create_dir_all(&self.port_dir)
            .with_context(|| format!("creating port dir {}", self.port_dir.display()))?;
        let lock_path = self.port_dir.join("daemon.lock");
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                use std::io::Write;
                writeln!(file, "{}", std::process::id()).ok();
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let holder_pid = std::fs::read_to_string(&lock_path)
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok());
                match holder_pid {
                    Some(pid) if is_pid_alive(pid) => {
                        bail!("another daemon is already running (pid {pid})");
                    }
                    stale => {
                        tracing::warn!(holder = ?stale, "stale daemon lock — taking over");
                        let _ = std::fs::remove_file(&lock_path);
                        let mut file = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&lock_path)
                            .context("taking over stale daemon lock")?;
                        use std::io::Write;
                        writeln!(file, "{}", std::process::id()).ok();
                    }
                }
            }
            Err(err) => return Err(err).context("opening daemon lock"),
        }
        Ok(Daemon {
            port_dir: self.port_dir,
            config: self.config,
            state: std::marker::PhantomData,
        })
    }
}

impl Daemon<Locked> {
    /// Bind an ephemeral loopback port and publish it (atomic rename).
    /// The listener travels with the state: serve() must use the SAME
    /// socket the port file points at.
    pub fn bind(self) -> Result<(Daemon<Bound>, TcpListener)> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).context("binding admin port")?;
        let port = listener.local_addr().context("reading local addr")?.port();
        let tmp = self.port_dir.join("admin.port.tmp");
        std::fs::write(&tmp, port.to_string()).context("writing port tmp")?;
        std::fs::rename(&tmp, self.port_dir.join("admin.port")).context("publishing port file")?;
        let bound = Daemon {
            port_dir: self.port_dir,
            config: self.config,
            state: std::marker::PhantomData,
        };
        Ok((bound, listener))
    }
}

impl Daemon<Bound> {
    /// Connect to PVE (stateless for token auth; logs in lazily for the
    /// password path) and become Ready.
    pub fn connect(self) -> Result<(Daemon<Ready>, PveClient)> {
        let client = PveClient::new(&self.config.pve)?;
        let ready = Daemon {
            port_dir: self.port_dir,
            config: self.config,
            state: std::marker::PhantomData,
        };
        Ok((ready, client))
    }
}

impl Daemon<Ready> {
    /// Serve the admin plane: one JSON-per-line connection, thread each.
    /// Never returns while healthy. The PVE client moves into the admin
    /// state when tools land; until then it is parked here.
    pub fn serve(self, listener: TcpListener, client: PveClient) -> Result<()> {
        let _client = client; // parked until the tool surface wires in
        let state = Arc::new(AdminState {
            config: self.config,
        });
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let state = state.clone();
                    std::thread::spawn(move || {
                        if let Err(err) = serve_connection(stream, &state) {
                            tracing::debug!(error = %err, "admin connection ended");
                        }
                    });
                }
                Err(err) => {
                    tracing::warn!(error = %err, "accept failed");
                    std::thread::sleep(POLL_INTERVAL);
                }
            }
        }
        Ok(())
    }
}

struct AdminState {
    // Read by tool handlers as they land; silenced until then.
    #[allow(dead_code)]
    config: Config,
}

// ---------------------------------------------------------------------------
// Client-side helpers (probe / spawn / wait)
// ---------------------------------------------------------------------------

pub fn port_dir() -> PathBuf {
    std::env::var_os("PVE_MCP_PORT_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("pve-mcp")))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn port_file() -> PathBuf {
    port_dir().join("admin.port")
}

pub fn connect_admin(timeout: Duration) -> Result<TcpStream> {
    let port: u16 = std::fs::read_to_string(port_file())?
        .trim()
        .parse()
        .context("parsing admin port file")?;
    let stream = TcpStream::connect(("127.0.0.1", port)).context("connecting to daemon")?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    Ok(stream)
}

pub fn is_daemon_listening() -> bool {
    connect_admin(Duration::from_secs(2)).is_ok()
}

/// Ensure a daemon is running: probe, then spawn detached, then wait for
/// readiness. Returns when the daemon accepts connections.
pub fn ensure_daemon(config_path: Option<&Path>) -> Result<()> {
    if is_daemon_listening() {
        return Ok(());
    }
    spawn_daemon_detached(config_path).context("spawning daemon")?;
    wait_for_ready()
}

fn spawn_daemon_detached(config_path: Option<&Path>) -> Result<()> {
    let exe = std::env::current_exe().context("resolving current exe")?;
    let mut args = String::from("daemon");
    if let Some(path) = config_path {
        args.push_str(&format!(" --config \"{}\"", path.display()));
    }
    // CRITICAL: detached spawns on Windows do not inherit the caller's
    // environment reliably; echo the port dir explicitly so the spawned
    // daemon writes its port where THIS caller is looking.
    if let Some(dir) = std::env::var_os("PVE_MCP_PORT_DIR") {
        args.push_str(&format!(" --port-dir \"{}\"", dir.to_string_lossy()));
    }
    #[cfg(windows)]
    {
        // Git Bash / terminal jobs kill children on close; DETACHED_PROCESS
        // + CREATE_BREAKAWAY still dies when the job forbids breakaway.
        // The reliable escape is WMI Win32_Process.Create — the WMI service
        // host creates the process, outside any caller job (herdr pattern,
        // same call shape as the wmi-rs doc example; know-mcp proven).
        let cmdline = format!("\"{}\" {}", exe.display(), args);
        let con = match wmi::WMIConnection::new(wmi::COMLibrary::new()?) {
            Ok(c) => c,
            Err(e) => bail!("WMI connection failed: {e}"),
        };
        let in_params = con
            .get_object("Win32_Process")?
            .get_method("Create")?
            .context("Win32_Process.Create method not found")?
            .spawn_instance()?;
        in_params.put_property("CommandLine", cmdline)?;
        let out = con.exec_method("Win32_Process", "Create", Some(&in_params))?;
        let ret = out
            .as_ref()
            .and_then(|o| o.get_property("ReturnValue").ok())
            .and_then(|v| match v {
                wmi::Variant::I4(v) => Some(v),
                wmi::Variant::UI4(v) => Some(v as i32),
                _ => None,
            })
            .unwrap_or(0);
        // Process creation is asynchronous — wait_for_ready() is the
        // authoritative check; treat 0/unknown as accepted and let the
        // probe loop confirm.
        if ret == 0 {
            return Ok(());
        }
        let mut props = String::new();
        if let Some(o) = &out {
            if let Ok(names) = o.list_properties() {
                for name in names {
                    if let Ok(val) = o.get_property(&name) {
                        props.push_str(&format!(" {name}={val:?}"));
                    }
                }
            }
        }
        bail!("WMI daemon spawn failed (ret={ret};{props})");
    }
    #[cfg(unix)]
    {
        let mut command = std::process::Command::new(&exe);
        command.arg("daemon");
        if let Some(path) = config_path {
            command.arg("--config").arg(path);
        }
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        {
            use std::os::unix::process::CommandExt;
            // SAFETY: setsid detaches the child from the controlling
            // terminal's process group.
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
        command.status().context("spawning detached daemon")?;
        Ok(())
    }
}

fn wait_for_ready() -> Result<()> {
    let deadline = std::time::Instant::now() + READY_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if is_daemon_listening() {
            return Ok(());
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    bail!("daemon did not become ready within {READY_TIMEOUT:?}");
}

pub fn admin_request(method: &str, params: Value) -> Result<Value> {
    let mut stream = connect_admin(Duration::from_secs(30))?;
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

// ---------------------------------------------------------------------------
// PID liveness (lock takeover decision)
// ---------------------------------------------------------------------------

// Windows FFI liveness probe: audited unsafe boundary (same call shape as
// the know-mcp daemon). Scope: open + zero-timeout wait + close on a pid.
#[cfg(windows)]
#[allow(unsafe_code)]
fn is_pid_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };
    let h = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if h.is_null() {
        // Our own pid is alive by definition even when OpenProcess fails.
        return pid == std::process::id();
    }
    let rc = unsafe { WaitForSingleObject(h, 0) }; // zero timeout: pure query
    unsafe { CloseHandle(h) };
    rc == WAIT_TIMEOUT // WAIT_OBJECT_0 (signaled) = process has exited
}

#[cfg(not(windows))]
#[allow(unsafe_code)]
fn is_pid_alive(pid: u32) -> bool {
    // kill(pid, 0): signal 0 is the documented existence probe; no signal
    // is delivered.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

// ---------------------------------------------------------------------------
// Connection serving
// ---------------------------------------------------------------------------

fn serve_connection(stream: TcpStream, state: &Arc<AdminState>) -> Result<()> {
    use std::io::{BufRead, BufReader, Write};
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(());
        }
        let Ok(request) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let reply = handle_admin(request, state).unwrap_or_else(|| {
            json!({"id": Value::Null, "error": "unhandled or malformed request"}).to_string()
        });
        writeln!(writer, "{reply}")?;
        writer.flush()?;
    }
}

fn handle_admin(request: Value, _state: &Arc<AdminState>) -> Option<String> {
    let method = request.get("method")?.as_str()?.to_string();
    let id = request.get("id").cloned();
    let _params = request.get("params").cloned().unwrap_or(Value::Null);
    let result: std::result::Result<Value, String> = match method.as_str() {
        "status" => Ok(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
            "tool_surface": "scaffold",
        })),
        "ping" => Ok(json!({"pong": true})),
        "stop" => {
            let _ = std::fs::remove_file(port_file());
            let _ = std::fs::remove_file(port_dir().join("daemon.lock"));
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(150));
                std::process::exit(0);
            });
            Ok(json!({"stopping": true}))
        }
        other => Err(format!("unknown method: {other}")),
    };
    let payload = match result {
        Ok(value) => json!({"id": id, "result": value}),
        Err(err) => json!({"id": id, "error": err}),
    };
    Some(payload.to_string())
}

// ---------------------------------------------------------------------------
// Run entry
// ---------------------------------------------------------------------------

pub fn run_daemon(config_path: Option<PathBuf>, force_port_dir: Option<PathBuf>) -> Result<()> {
    let config = Config::load(config_path.as_deref())?;
    let dir = force_port_dir.unwrap_or_else(port_dir);

    let (bound, listener) = Daemon::new(dir.clone(), config).acquire_lock()?.bind()?;
    let port = listener.local_addr()?.port();
    tracing::info!(port, "admin plane bound");

    let (ready, client) = bound.connect()?;
    tracing::info!(dir = %dir.display().to_string(), "daemon ready");
    ready.serve(listener, client)
}
