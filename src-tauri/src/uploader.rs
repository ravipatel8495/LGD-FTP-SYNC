// uploader.rs — Rust port of the old uploader.js: one small wrapper around
// all connection types:
//   "ftp"           plain FTP           (port 21)
//   "ftps"          explicit FTPS / TLS (port 21)
//   "ftps-implicit" implicit FTPS       (port 990)
//   "sftp"          SFTP over SSH       (port 22)
use crate::config::FtpConfig;
use russh::client::{self, Handler};
use russh::keys::PublicKeyOrCertificate;
use russh::ChannelId;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use std::io::{self, Cursor, Read};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use suppaftp::native_tls::TlsConnector;
use suppaftp::types::FileType;
use suppaftp::{FtpError, ImplFtpStream, Mode, NativeTlsConnector, NativeTlsFtpStream, TlsStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const DATA_TIMEOUT: Duration = Duration::from_secs(3);
const IO_TIMEOUT: Duration = Duration::from_secs(180);
const TEST_TIMEOUT: Duration = Duration::from_secs(25);

fn default_port(protocol: &str) -> u16 {
    match protocol {
        "ftps-implicit" => 990,
        "sftp" => 22,
        _ => 21,
    }
}

fn normalize_port(config: &FtpConfig) -> u16 {
    config
        .port
        .parse::<u16>()
        .ok()
        .filter(|p| *p > 0)
        .unwrap_or_else(|| default_port(&config.protocol))
}

// Always use forward slashes on the remote side, even when the local
// machine is Windows.
fn to_remote_path(remote_dir: &str, relative_path: &str) -> String {
    let rel = relative_path.replace('\\', "/");
    let base = remote_dir.trim_end_matches('/');
    format!("{}/{}", base, rel.trim_start_matches('/'))
}

pub fn product_type_dir(product_type: &str) -> &'static str {
    match product_type.to_uppercase().as_str() {
        "NATURAL" => "NATURAL",
        _ => "LGD",
    }
}

fn with_product_prefix(product_type: &str, relative_path: &str) -> String {
    let prefix = product_type_dir(product_type);
    let rel = relative_path.replace('\\', "/");
    let rel = rel.trim_start_matches('/');
    let upper = rel.to_uppercase();
    if upper.starts_with("LGD/") || upper.starts_with("NATURAL/") || upper == "LGD" || upper == "NATURAL"
    {
        rel.to_string()
    } else {
        format!("{prefix}/{rel}")
    }
}

fn remote_dirname(remote_file: &str) -> &str {
    match remote_file.rfind('/') {
        Some(0) => "/",
        Some(idx) => &remote_file[..idx],
        None => "/",
    }
}

// ---------------- FTP / FTPS (suppaftp, blocking API) ----------------

fn ftp_login<T: TlsStream>(
    stream: &mut ImplFtpStream<T>,
    config: &FtpConfig,
) -> Result<(), String> {
    stream
        .login(&config.user, &config.password)
        .map_err(|e| e.to_string())
}

fn ftp_ensure_dir<T: TlsStream>(stream: &mut ImplFtpStream<T>, remote_folder: &str) {
    let _ = stream.cwd("/");
    if remote_folder.is_empty() || remote_folder == "/" {
        return;
    }
    for part in remote_folder.split('/').filter(|p| !p.is_empty()) {
        let _ = stream.mkdir(part);
        if stream.cwd(part).is_err() {
            let abs = format!("/{part}");
            let _ = stream.mkdir(&abs);
            let _ = stream.cwd(&abs);
        }
    }
}

fn ftp_cwd_folder<T: TlsStream>(
    stream: &mut ImplFtpStream<T>,
    remote_folder: &str,
) -> Result<(), String> {
    ftp_ensure_dir(stream, remote_folder);
    if remote_folder.is_empty() || remote_folder == "/" {
        return stream.cwd("/").map_err(|e| e.to_string());
    }
    if stream.cwd(remote_folder).is_ok() {
        return Ok(());
    }
    let _ = stream.cwd("/");
    for part in remote_folder.split('/').filter(|p| !p.is_empty()) {
        let _ = stream.mkdir(part);
        stream
            .cwd(part)
            .map_err(|e| format!("Cannot open remote folder {remote_folder}: {e}"))?;
    }
    Ok(())
}

fn tls_connector() -> Result<NativeTlsConnector, String> {
    // Same permissiveness as the old Electron client's
    // `rejectUnauthorized: false` — many internal FTP servers use
    // self-signed certs.
    let tls = TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(NativeTlsConnector::from(tls))
}

fn resolve_ftp_addr(config: &FtpConfig) -> Result<SocketAddr, String> {
    let host = config.host.trim();
    if host.is_empty() {
        return Err("Please enter a host name.".into());
    }
    let port = normalize_port(config);
    let addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("Could not resolve {host}: {e}"))?
        .collect();
    addrs
        .iter()
        .copied()
        .find(|a| a.is_ipv4())
        .or_else(|| addrs.into_iter().next())
        .ok_or_else(|| format!("Could not resolve {host}"))
}

fn pasv_blocked_message(host: &str, err: &str) -> String {
    format!(
        "{err}. Login on port 21 works, but FTP data ports 1024-1224 on {host} are not reachable. Open TCP 1024-1224 on the server firewall/security group (Docker -p is not enough), and set ftp_server_config.host to the public hostname — not 127.0.0.1."
    )
}

fn apply_socket_timeouts(stream: &NativeTlsFtpStream) {
    let sock = stream.get_ref();
    let _ = sock.set_read_timeout(Some(IO_TIMEOUT));
    let _ = sock.set_write_timeout(Some(IO_TIMEOUT));
    let _ = sock.set_nodelay(true);
}

fn prepare_data_stream(s: TcpStream) -> TcpStream {
    let _ = s.set_read_timeout(Some(IO_TIMEOUT));
    let _ = s.set_write_timeout(Some(IO_TIMEOUT));
    let _ = s.set_nodelay(true);
    s
}

fn push_resolved(out: &mut Vec<SocketAddr>, host: &str, port: u16) {
    if let Ok(addrs) = (host, port).to_socket_addrs() {
        let mut v4 = Vec::new();
        let mut v6 = Vec::new();
        for addr in addrs {
            if addr.is_ipv4() {
                v4.push(addr);
            } else {
                v6.push(addr);
            }
        }
        for addr in v4.into_iter().chain(v6) {
            if !out.contains(&addr) {
                out.push(addr);
            }
        }
    }
}

fn open_data_stream(host: &str, data_addr: SocketAddr) -> Result<TcpStream, FtpError> {
    let port = data_addr.port();
    let mut candidates = Vec::new();
    // Same host the user typed for login — required for Docker ftp-srv, which
    // often advertises 127.0.0.1 or a public IP that this Mac cannot hairpin.
    push_resolved(&mut candidates, host, port);
    if !candidates.contains(&data_addr) {
        candidates.push(data_addr);
    }
    let mut last = None;
    for addr in candidates {
        match TcpStream::connect_timeout(&addr, DATA_TIMEOUT) {
            Ok(s) => return Ok(prepare_data_stream(s)),
            Err(e) => last = Some(e),
        }
    }
    Err(FtpError::ConnectionError(last.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            format!("FTP data port {port} timed out (Docker PASV range is 1024-1224)"),
        )
    })))
}

fn ftp_connect(config: &FtpConfig) -> Result<NativeTlsFtpStream, String> {
    ftp_connect_mode(config, Mode::Passive)
}

fn ftp_connect_mode(config: &FtpConfig, mode: Mode) -> Result<NativeTlsFtpStream, String> {
    let addr = resolve_ftp_addr(config)?;
    let connector = tls_connector()?;
    let mut stream = if config.protocol == "ftps-implicit" {
        NativeTlsFtpStream::connect_secure_implicit(addr, connector, &config.host)
            .map_err(|e| format!("Could not connect to {addr} (implicit FTPS): {e}"))?
    } else {
        let s = NativeTlsFtpStream::connect_timeout(addr, CONNECT_TIMEOUT)
            .map_err(|e| format!("Could not connect to {addr}: {e}"))?;
        apply_socket_timeouts(&s);
        if config.protocol == "ftps" {
            s.into_secure(connector, &config.host)
                .map_err(|e| format!("FTPS handshake failed: {e}"))?
        } else {
            s
        }
    };
    let host = config.host.clone();
    stream = stream.passive_stream_builder(move |data_addr| open_data_stream(&host, data_addr));
    apply_socket_timeouts(&stream);
    stream.set_mode(mode);
    // Off: ftp-srv PASV may send 127.0.0.1 while login used a hostname.
    // Rewriting to the control-channel peer IP breaks local Docker hairpin.
    stream.set_passive_nat_workaround(false);
    Ok(stream)
}

fn product_dirs_to_check(_config: &FtpConfig) -> Vec<&'static str> {
    vec!["LGD", "NATURAL"]
}

fn product_dirs(config: &FtpConfig) -> Vec<String> {
    product_dirs_to_check(config)
        .into_iter()
        .map(|d| format!("/{d}"))
        .collect()
}

/// Login once, then create/open each product folder. Do not LIST — PASV
/// listing often hangs on internal FTP servers and makes Test FTP look stuck.
fn ftp_probe(config: &FtpConfig, dirs: &[String]) -> Result<Vec<String>, String> {
    let mut stream = ftp_connect(config)?;
    ftp_login(&mut stream, config).map_err(|e| format!("Login failed: {e}"))?;
    let mut parts = Vec::new();
    for remote_dir in dirs {
        match ftp_cwd_folder(&mut stream, remote_dir) {
            Ok(()) => parts.push(format!("{remote_dir} ready")),
            Err(e) => parts.push(format!("{remote_dir} not available ({e})")),
        }
        let _ = stream.cwd("/");
    }
    if let Err(err) = stream.nlst(None) {
        let _ = stream.quit();
        return Err(pasv_blocked_message(&config.host, &err.to_string()));
    }
    let _ = stream.quit();
    Ok(parts)
}

pub(crate) fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    if n < 1024 {
        format!("{n} B")
    } else if (n as f64) < MB {
        format!("{:.1} KB", n as f64 / KB)
    } else {
        format!("{:.2} MB", n as f64 / MB)
    }
}

pub type ProgressFn = Arc<dyn Fn(u64, u64) + Send + Sync>;

struct ProgressReader<R, F>
where
    R: Read,
    F: Fn(u64, u64),
{
    inner: R,
    sent: u64,
    total: u64,
    last_pct: u8,
    on_progress: F,
}

impl<R, F> Read for ProgressReader<R, F>
where
    R: Read,
    F: Fn(u64, u64),
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 {
            self.sent += n as u64;
            let pct = if self.total == 0 {
                100
            } else {
                ((self.sent.saturating_mul(100)) / self.total).min(100) as u8
            };
            if pct != self.last_pct {
                self.last_pct = pct;
                (self.on_progress)(self.sent, self.total);
            }
        }
        Ok(n)
    }
}

fn report_progress(on_progress: &Option<ProgressFn>, sent: u64, total: u64) {
    if let Some(cb) = on_progress {
        cb(sent, total);
    }
}

fn ftp_filename(remote_file: &str) -> &str {
    remote_file.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or(remote_file)
}

fn ftp_put(
    config: &FtpConfig,
    local_file: &Path,
    remote_folder: &str,
    remote_file: &str,
    on_progress: Option<ProgressFn>,
) -> Result<(), String> {
    let data = std::fs::read(local_file).map_err(|e| e.to_string())?;
    let total = data.len() as u64;
    report_progress(&on_progress, 0, total);
    let name = ftp_filename(remote_file).to_string();
    let mut last_err = String::new();
    // Passive only. Active PORT makes ftp-srv return
    // "500 The given address is not yours". Prefer classic PASV so the
    // data host comes from the server (or our login-host fallback).
    for mode in [Mode::Passive, Mode::ExtendedPassive] {
        match ftp_put_once(
            config,
            remote_folder,
            &name,
            &data,
            total,
            on_progress.clone(),
            mode,
        ) {
            Ok(()) => {
                report_progress(&on_progress, total, total);
                return Ok(());
            }
            Err(err) => last_err = err,
        }
    }
    Err(pasv_blocked_message(&config.host, &last_err))
}

fn ftp_put_once(
    config: &FtpConfig,
    remote_folder: &str,
    filename: &str,
    data: &[u8],
    total: u64,
    on_progress: Option<ProgressFn>,
    mode: Mode,
) -> Result<(), String> {
    let mut stream = ftp_connect_mode(config, mode)?;
    ftp_login(&mut stream, config).map_err(|e| format!("Login failed: {e}"))?;
    let _ = stream.transfer_type(FileType::Binary);
    ftp_cwd_folder(&mut stream, remote_folder)?;
    let cb = on_progress.clone();
    let mut reader = ProgressReader {
        inner: Cursor::new(data),
        sent: 0,
        total,
        last_pct: 255,
        on_progress: move |sent, total| report_progress(&cb, sent, total),
    };
    let result = stream
        .put_file(filename, &mut reader)
        .map_err(|e| e.to_string());
    let _ = stream.quit();
    result?;
    Ok(())
}

// ---------------- SFTP (russh + russh-sftp, async) ----------------

struct SshClient;

impl Handler for SshClient {
    type Error = russh::Error;

    // Same permissiveness as the FTPS side's `rejectUnauthorized: false`:
    // this app trusts whatever host it's pointed at, it doesn't pin keys.
    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        _data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

async fn sftp_connect(config: &FtpConfig) -> Result<SftpSession, String> {
    let ssh_config = Arc::new(client::Config::default());
    let port = normalize_port(config);
    let addr = (config.host.as_str(), port);
    let mut session = tokio::time::timeout(CONNECT_TIMEOUT, client::connect(ssh_config, addr, SshClient))
        .await
        .map_err(|_| format!("Could not reach {}:{} (SFTP timed out)", config.host, port))?
        .map_err(|e| e.to_string())?;
    let authenticated = session
        .authenticate_password(&config.user, &config.password)
        .await
        .map_err(|e| e.to_string())?;
    if !authenticated.success() {
        return Err("Authentication failed".to_string());
    }
    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| e.to_string())?;
    SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| e.to_string())
}

async fn sftp_probe(config: &FtpConfig, dirs: &[String]) -> Result<Vec<String>, String> {
    let sftp = sftp_connect(config).await?;
    let mut parts = Vec::new();
    for remote_dir in dirs {
        if remote_dir != "/" {
            sftp_ensure_dir(&sftp, remote_dir).await;
        }
        match sftp.metadata(remote_dir.as_str()).await {
            Ok(_) => parts.push(format!("{remote_dir} ready")),
            Err(e) => parts.push(format!("{remote_dir} not available ({e})")),
        }
    }
    Ok(parts)
}

async fn sftp_ensure_dir(sftp: &SftpSession, remote_folder: &str) {
    let mut path = String::new();
    for part in remote_folder.split('/').filter(|p| !p.is_empty()) {
        path.push('/');
        path.push_str(part);
        let _ = sftp.create_dir(&path).await;
    }
}

async fn sftp_put(
    config: &FtpConfig,
    local_file: &Path,
    remote_folder: &str,
    remote_file: &str,
    on_progress: Option<ProgressFn>,
) -> Result<(), String> {
    let sftp = sftp_connect(config).await?;
    if remote_folder != "/" {
        sftp_ensure_dir(&sftp, remote_folder).await;
    }
    let total = tokio::fs::metadata(local_file)
        .await
        .map_err(|e| e.to_string())?
        .len();
    let mut src = tokio::fs::File::open(local_file)
        .await
        .map_err(|e| e.to_string())?;
    let mut file = sftp
        .open_with_flags(
            remote_file,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await
        .map_err(|e| e.to_string())?;
    report_progress(&on_progress, 0, total);
    let mut buf = vec![0u8; 64 * 1024];
    let mut sent = 0u64;
    let mut last_pct = 255u8;
    loop {
        let n = src.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).await.map_err(|e| e.to_string())?;
        sent += n as u64;
        let pct = if total == 0 {
            100
        } else {
            ((sent.saturating_mul(100)) / total).min(100) as u8
        };
        if pct != last_pct {
            last_pct = pct;
            report_progress(&on_progress, sent, total);
        }
    }
    file.shutdown().await.map_err(|e| e.to_string())?;
    report_progress(&on_progress, total, total);
    Ok(())
}

// ---------------- public API ----------------

// Test the connection by logging in once, then opening /LGD and /NATURAL.
pub async fn test_connection(config: FtpConfig) -> Result<String, String> {
    let dirs = product_dirs(&config);
    if config.protocol == "sftp" {
        let parts = tokio::time::timeout(TEST_TIMEOUT, sftp_probe(&config, &dirs))
            .await
            .map_err(|_| {
                "SFTP timed out after 25 seconds. Check host, port, and connection type.".to_string()
            })??;
        Ok(format!("Connected via SFTP. {}", parts.join(". ")))
    } else {
        let proto = config.protocol.to_uppercase();
        let config2 = config.clone();
        let dirs2 = dirs.clone();
        let parts = tokio::time::timeout(
            TEST_TIMEOUT,
            tokio::task::spawn_blocking(move || ftp_probe(&config2, &dirs2)),
        )
        .await
        .map_err(|_| {
            format!("{proto} timed out after 25 seconds. Check host, port, and connection type.")
        })?
        .map_err(|e| e.to_string())??;
        Ok(format!("Connected via {proto}. {}", parts.join(". ")))
    }
}

/// Create /LGD and/or /NATURAL on the server so the first upload is not lost.
pub async fn ensure_product_dirs(config: &FtpConfig, types: &[String]) -> Result<(), String> {
    let mut dirs: Vec<String> = types
        .iter()
        .map(|t| format!("/{}", product_type_dir(t)))
        .collect();
    dirs.sort();
    dirs.dedup();
    if dirs.is_empty() {
        dirs.push("/LGD".into());
        dirs.push("/NATURAL".into());
    }
    if config.protocol == "sftp" {
        let _ = sftp_probe(config, &dirs).await?;
        Ok(())
    } else {
        let config2 = config.clone();
        tokio::task::spawn_blocking(move || ftp_probe(&config2, &dirs))
            .await
            .map_err(|e| e.to_string())??;
        Ok(())
    }
}

#[allow(dead_code)]
pub async fn upload_file(
    config: FtpConfig,
    local_file: PathBuf,
    relative_path: String,
) -> Result<String, String> {
    let product = config.normalized_product_type();
    upload_file_as(config, local_file, relative_path, &product, None).await
}

pub async fn upload_file_as(
    config: FtpConfig,
    local_file: PathBuf,
    relative_path: String,
    product_type: &str,
    on_progress: Option<ProgressFn>,
) -> Result<String, String> {
    let prefixed = with_product_prefix(product_type, &relative_path);
    let remote_file = to_remote_path("/", &prefixed);
    let remote_folder = remote_dirname(&remote_file).to_string();

    if config.protocol == "sftp" {
        sftp_put(
            &config,
            &local_file,
            &remote_folder,
            &remote_file,
            on_progress,
        )
        .await?;
    } else {
        let config2 = config.clone();
        let remote_folder2 = remote_folder.clone();
        let remote_file2 = remote_file.clone();
        tokio::task::spawn_blocking(move || {
            ftp_put(
                &config2,
                &local_file,
                &remote_folder2,
                &remote_file2,
                on_progress,
            )
        })
        .await
        .map_err(|e| e.to_string())??;
    }
    Ok(remote_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "sku,metal,purity,qty\nLGD-001,Gold,24K,2\nLGD-002,Gold,22K,1\n";

    fn cfg_from_env() -> Option<FtpConfig> {
        Some(FtpConfig {
            protocol: std::env::var("FTP_PROTOCOL").unwrap_or_else(|_| "ftp".into()),
            host: std::env::var("FTP_HOST").ok()?,
            port: std::env::var("FTP_PORT").unwrap_or_else(|_| "21".into()),
            user: std::env::var("FTP_USER").ok()?,
            password: std::env::var("FTP_PASSWORD").ok()?,
            local_path: String::new(),
            product_type: "LGD".into(),
            local_paths: crate::config::LocalPaths::default(),
            watching: false,
        })
    }

    fn write_temp_csv(name: &str, body: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, body).expect("write temp csv");
        path
    }

    fn ftp_nlst(config: &FtpConfig, dir: &str) -> Result<Vec<String>, String> {
        let mut stream = ftp_connect(config)?;
        ftp_login(&mut stream, config)?;
        let names = stream.nlst(Some(dir)).map_err(|e| e.to_string())?;
        let _ = stream.quit();
        Ok(names)
    }

    fn ftp_retr(config: &FtpConfig, remote: &str) -> Result<Vec<u8>, String> {
        let mut stream = ftp_connect(config)?;
        ftp_login(&mut stream, config)?;
        let _ = stream.cwd("/");
        if let Some((folder, _)) = remote.rsplit_once('/').filter(|(d, _)| !d.is_empty()) {
            if folder != "/" && !folder.is_empty() {
                stream.cwd(folder).map_err(|e| e.to_string())?;
            }
        }
        let name = remote.rsplit('/').next().unwrap_or(remote);
        let cursor = stream.retr_as_buffer(name).map_err(|e| e.to_string())?;
        let _ = stream.quit();
        Ok(cursor.into_inner())
    }

    #[test]
    fn port_defaults_match_ui() {
        assert_eq!(default_port("ftp"), 21);
        assert_eq!(default_port("ftps"), 21);
        assert_eq!(default_port("ftps-implicit"), 990);
        assert_eq!(default_port("sftp"), 22);
        let cfg = FtpConfig {
            protocol: "ftp".into(),
            host: "127.0.0.1".into(),
            port: String::new(),
            user: String::new(),
            password: String::new(),
            local_path: String::new(),
            product_type: "LGD".into(),
            local_paths: crate::config::LocalPaths::default(),
            watching: false,
        };
        assert_eq!(normalize_port(&cfg), 21);
    }

    #[test]
    fn remote_paths_always_use_forward_slashes() {
        assert_eq!(to_remote_path("/", "a.csv"), "/a.csv");
        assert_eq!(to_remote_path("/", "sub\\nested\\a.csv"), "/sub/nested/a.csv");
        assert_eq!(to_remote_path("/", "sub/a.csv"), "/sub/a.csv");
        assert_eq!(remote_dirname("/a.csv"), "/");
        assert_eq!(remote_dirname("/sub/nested/a.csv"), "/sub/nested");
        assert_eq!(with_product_prefix("LGD", "a.csv"), "LGD/a.csv");
        assert_eq!(with_product_prefix("NATURAL", "a.csv"), "NATURAL/a.csv");
        assert_eq!(with_product_prefix("LGD", "LGD/a.csv"), "LGD/a.csv");
    }

    #[tokio::test]
    async fn ftp_manual_test_suite() {
        let Some(config) = cfg_from_env() else {
            eprintln!("skipping: set FTP_HOST / FTP_USER / FTP_PASSWORD");
            return;
        };

        println!("TC1 Test FTP (valid credentials)");
        let msg = test_connection(config.clone())
            .await
            .expect("TC1: Test FTP should succeed");
        assert!(msg.contains("Connected via FTP"), "TC1 message: {msg}");
        println!("  PASS {msg}");

        println!("TC2 Test FTP (wrong password must fail)");
        let mut bad = config.clone();
        bad.password = "wrong-password".into();
        let err = test_connection(bad)
            .await
            .expect_err("TC2: bad password should fail");
        assert!(!err.is_empty());
        println!("  PASS rejected: {err}");

        println!("TC3 Upload CSV to FTP root");
        let local = write_temp_csv("lgd-manual-test.csv", CSV);
        let remote = upload_file(config.clone(), local.clone(), "lgd-manual-test.csv".into())
            .await
            .expect("TC3: CSV upload should succeed");
        assert_eq!(remote, "/LGD/lgd-manual-test.csv");
        println!("  PASS uploaded {remote}");

        println!("TC4 Download CSV and compare bytes");
        let cfg = config.clone();
        let remote2 = remote.clone();
        let expected = CSV.as_bytes().to_vec();
        let got = tokio::task::spawn_blocking(move || ftp_retr(&cfg, &remote2))
            .await
            .unwrap()
            .expect("TC4: download should succeed");
        assert_eq!(got, expected, "TC4: remote CSV must match local");
        println!("  PASS {} bytes match", got.len());

        println!("TC5 Nested folder upload (subfolders recreated on server)");
        let nested_local = write_temp_csv("lgd-nested.csv", CSV);
        let nested_remote = upload_file(
            config.clone(),
            nested_local,
            "lgd-watch/sub/lgd-nested.csv".into(),
        )
        .await
        .expect("TC5: nested upload should succeed");
        assert_eq!(nested_remote, "/LGD/lgd-watch/sub/lgd-nested.csv");
        let cfg = config.clone();
        let names = tokio::task::spawn_blocking(move || ftp_nlst(&cfg, "/LGD/lgd-watch/sub"))
            .await
            .unwrap()
            .expect("TC5: list nested dir");
        assert!(
            names.iter().any(|n| n.ends_with("lgd-nested.csv")),
            "TC5: {names:?}"
        );
        println!("  PASS listed {names:?}");

        println!("TC6 Overwrite existing CSV");
        let updated = "sku,qty\nLGD-001,99\n";
        let first = write_temp_csv("lgd-overwrite.csv", CSV);
        upload_file(config.clone(), first, "lgd-overwrite.csv".into())
            .await
            .expect("TC6: first upload should succeed");
        let overwrite = write_temp_csv("lgd-overwrite.csv", updated);
        upload_file(config.clone(), overwrite, "lgd-overwrite.csv".into())
            .await
            .expect("TC6: overwrite should succeed");
        let cfg = config.clone();
        let got = tokio::task::spawn_blocking(move || ftp_retr(&cfg, "/LGD/lgd-overwrite.csv"))
            .await
            .unwrap()
            .expect("TC6: re-download");
        assert_eq!(got, updated.as_bytes());
        println!("  PASS overwrite verified");

        println!("TC7 Start-watching equivalent: upload existing files, skip hidden");
        let watch_root = std::env::temp_dir().join("lgd-watch-root");
        let _ = std::fs::remove_dir_all(&watch_root);
        std::fs::create_dir_all(watch_root.join("batch")).unwrap();
        std::fs::write(watch_root.join("visible.csv"), CSV).unwrap();
        std::fs::write(watch_root.join("batch/inner.csv"), CSV).unwrap();
        std::fs::write(watch_root.join(".hidden.csv"), "secret\n").unwrap();
        let mut existing = Vec::new();
        crate::watcher::collect_existing_files_for_test(&watch_root, &watch_root, &mut existing);
        let names: Vec<String> = existing
            .iter()
            .map(|p| {
                p.strip_prefix(&watch_root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert!(names.iter().any(|n| n == "visible.csv"), "{names:?}");
        assert!(names.iter().any(|n| n == "batch/inner.csv"), "{names:?}");
        assert!(!names.iter().any(|n| n.contains("hidden")), "{names:?}");
        for path in existing {
            let relative = path
                .strip_prefix(&watch_root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            upload_file(config.clone(), path, format!("lgd-watch-root/{relative}"))
                .await
                .unwrap_or_else(|e| panic!("TC7 upload {relative}: {e}"));
        }
        println!("  PASS uploaded {:?}, skipped hidden", names);

        println!("ALL FTP MANUAL CASES PASSED");
    }
}
