use ssh2::Session;
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::Duration;

use crate::config::Config;

pub struct SshClient {
    session: Option<Session>,
    videos_dir: String,
}

impl SshClient {
    pub fn new() -> Self {
        SshClient {
            session: None,
            videos_dir: String::new(),
        }
    }

    pub fn connect(&mut self, config: &Config) -> Result<(), String> {
        let addr_str = format!("{}:{}", config.host, config.port);
        let addr = addr_str
            .to_socket_addrs()
            .map_err(|e| format!("Invalid address '{}': {}", addr_str, e))?
            .next()
            .ok_or_else(|| format!("Could not resolve '{}'", addr_str))?;

        let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
            .map_err(|e| format!("Connection failed ({}): {}", addr_str, e))?;

        // Prevent hangs on slow/broken connections
        let _ = tcp.set_read_timeout(Some(Duration::from_secs(60)));
        let _ = tcp.set_write_timeout(Some(Duration::from_secs(30)));

        let mut session =
            Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;
        session.set_tcp_stream(tcp);
        session
            .handshake()
            .map_err(|e| format!("SSH handshake failed: {}", e))?;

        try_authenticate(&mut session, &config.user, config.key_path.as_deref())?;

        self.session = Some(session);
        self.videos_dir = {
            let mut dir = config.videos_dir.clone();
            if !dir.ends_with('/') {
                dir.push('/');
            }
            dir
        };
        Ok(())
    }

    pub fn list_videos(&self) -> Result<Vec<String>, String> {
        let session = self.session.as_ref().ok_or("Not connected")?;
        let sftp = session.sftp().map_err(|e| format!("SFTP error: {}", e))?;
        let entries = sftp
            .readdir(std::path::Path::new(&self.videos_dir))
            .map_err(|e| format!("Failed to list '{}': {}", self.videos_dir, e))?;

        let video_extensions = ["mp4", "mkv", "webm", "avi", "mov", "flv"];
        let mut videos: Vec<String> = entries
            .into_iter()
            .filter_map(|(path, _)| {
                let filename = path.file_name()?.to_string_lossy().to_string();
                let ext = path.extension()?.to_string_lossy().to_lowercase();
                if video_extensions.contains(&ext.as_str()) {
                    Some(filename)
                } else {
                    None
                }
            })
            .collect();

        videos.sort();
        Ok(videos)
    }

    pub fn get_thumbnail(&self, filename: &str) -> Result<Vec<u8>, String> {
        let session = self.session.as_ref().ok_or("Not connected")?;

        let escaped = filename.replace('\'', "'\\''");
        let video_path = format!("{}{}", self.videos_dir, escaped);
        let cmd = format!(
            "ffmpeg -y -i '{}' -ss 00:00:01 -vframes 1 -vf scale=280:-1 -f image2pipe -vcodec mjpeg - 2>/dev/null",
            video_path
        );

        let mut channel = session
            .channel_session()
            .map_err(|e| format!("Channel failed: {}", e))?;
        channel
            .exec(&cmd)
            .map_err(|e| format!("Exec failed: {}", e))?;

        let mut data = Vec::new();
        channel
            .read_to_end(&mut data)
            .map_err(|e| format!("Read failed: {}", e))?;

        let _ = channel.send_eof();
        let _ = channel.wait_eof();
        let _ = channel.close();
        let _ = channel.wait_close();

        if data.is_empty() {
            return Err("No thumbnail generated".to_string());
        }
        Ok(data)
    }

    /// Lists subdirectories at `path` on the VPS. Returns sorted directory names.
    pub fn list_dirs(&self, path: &str) -> Result<Vec<String>, String> {
        let session = self.session.as_ref().ok_or("Not connected")?;
        let sftp = session.sftp().map_err(|e| format!("SFTP error: {}", e))?;

        let entries = sftp
            .readdir(std::path::Path::new(path))
            .map_err(|e| format!("Cannot read '{}': {}", path, e))?;

        let mut dirs: Vec<String> = entries
            .into_iter()
            .filter_map(|(entry_path, stat)| {
                let name = entry_path.file_name()?.to_string_lossy().to_string();
                if name == "." || name == ".." {
                    return None;
                }
                // S_IFDIR = 0o040000
                let is_dir = stat.perm
                    .map(|p| p & 0o170000 == 0o040000)
                    .unwrap_or(false);
                if is_dir { Some(name) } else { None }
            })
            .collect();

        dirs.sort();
        Ok(dirs)
    }

    pub fn delete_video(&self, filename: &str) -> Result<(), String> {
        let session = self.session.as_ref().ok_or("Not connected")?;
        let sftp = session.sftp().map_err(|e| format!("SFTP error: {}", e))?;
        let path = format!("{}{}", self.videos_dir, filename);
        sftp.unlink(std::path::Path::new(&path))
            .map_err(|e| format!("Delete failed: {}", e))
    }

    pub fn is_connected(&self) -> bool {
        self.session.is_some()
    }
}

impl Default for SshClient {
    fn default() -> Self {
        Self::new()
    }
}

/// One-time credential setup:
/// 1. Generates `~/.ssh/xero_yt_manager` ed25519 key pair (skipped if already exists).
/// 2. Connects to the VPS using password auth.
/// 3. Appends the public key to `~/.ssh/authorized_keys` on the VPS (idempotent).
/// Returns the path to the private key file on success.
pub fn setup_key_auth(host: &str, port: u16, user: &str, password: &str) -> Result<PathBuf, String> {
    // ── 1. Prepare ~/.ssh directory ──────────────────────────────────────────
    let ssh_dir = dirs::home_dir()
        .ok_or("Cannot find home directory")?
        .join(".ssh");
    std::fs::create_dir_all(&ssh_dir)
        .map_err(|e| format!("Cannot create ~/.ssh: {}", e))?;

    let key_path = ssh_dir.join("xero_yt_manager");
    let pub_key_path = ssh_dir.join("xero_yt_manager.pub");

    // ── 2. Generate ed25519 key pair if not already present ──────────────────
    if !key_path.exists() {
        let output = std::process::Command::new("ssh-keygen")
            .args([
                "-t", "ed25519",
                "-f", &key_path.to_string_lossy(),
                "-N", "",           // no passphrase
                "-C", "xero-yt-manager",
                "-q",               // quiet
            ])
            .output()
            .map_err(|e| format!("ssh-keygen not found: {}. Install openssh-client.", e))?;

        if !output.status.success() {
            return Err(format!(
                "Key generation failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }

    // ── 3. Read the public key ────────────────────────────────────────────────
    let pub_key = std::fs::read_to_string(&pub_key_path)
        .map_err(|e| format!("Cannot read public key: {}", e))?;
    let pub_key = pub_key.trim().to_string();

    // ── 4. Connect to VPS using password auth ────────────────────────────────
    let addr_str = format!("{}:{}", host, port);
    let addr = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("Invalid address '{}': {}", addr_str, e))?
        .next()
        .ok_or_else(|| format!("Cannot resolve '{}'", addr_str))?;

    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connection failed: {}", e))?;
    let _ = tcp.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = tcp.set_write_timeout(Some(Duration::from_secs(30)));

    let mut session = Session::new()
        .map_err(|e| format!("SSH session error: {}", e))?;
    session.set_tcp_stream(tcp);
    session.handshake()
        .map_err(|e| format!("SSH handshake failed: {}", e))?;

    session.userauth_password(user, password)
        .map_err(|e| format!("Password authentication failed: {}", e))?;

    if !session.authenticated() {
        return Err("Password was rejected by the server.".to_string());
    }

    // ── 5. Install public key on server (idempotent via grep check) ──────────
    // SSH public keys are base64 + space-separated — safe to single-quote in shell.
    let cmd = format!(
        "mkdir -p ~/.ssh && chmod 700 ~/.ssh && \
         (grep -qxF '{key}' ~/.ssh/authorized_keys 2>/dev/null || \
          echo '{key}' >> ~/.ssh/authorized_keys) && \
         chmod 600 ~/.ssh/authorized_keys",
        key = pub_key
    );

    let mut channel = session.channel_session()
        .map_err(|e| format!("Channel error: {}", e))?;
    channel.exec(&cmd)
        .map_err(|e| format!("Remote command failed: {}", e))?;

    let mut out = Vec::new();
    let _ = channel.read_to_end(&mut out);
    let _ = channel.send_eof();
    let _ = channel.wait_eof();
    let _ = channel.close();
    let _ = channel.wait_close();

    let exit_code = channel.exit_status().unwrap_or(-1);
    if exit_code != 0 {
        return Err(format!(
            "Failed to install SSH key on server (exit {}). Check ~/.ssh/ permissions.",
            exit_code
        ));
    }

    Ok(key_path)
}

/// Public entry point used by download_manager for its own SSH session.
pub fn authenticate_session(
    session: &mut Session,
    user: &str,
    key_path: Option<&str>,
) -> Result<(), String> {
    try_authenticate(session, user, key_path)
}

/// Tries all available SSH keys. Tries the configured key_path first,
/// then falls back to standard ~/.ssh/ key names.
fn try_authenticate(
    session: &mut Session,
    user: &str,
    key_path: Option<&str>,
) -> Result<(), String> {
    let ssh_dir = dirs::home_dir()
        .ok_or_else(|| "Cannot find home directory".to_string())?
        .join(".ssh");

    // Try the explicitly configured key first
    if let Some(kp) = key_path {
        let priv_key = std::path::Path::new(kp);
        if priv_key.exists() {
            let pub_str = format!("{}.pub", kp);
            let pub_opt: Option<&std::path::Path> = if std::path::Path::new(&pub_str).exists() {
                Some(std::path::Path::new(&pub_str))
            } else {
                None
            };
            if session
                .userauth_pubkey_file(user, pub_opt, priv_key, None)
                .is_ok()
                && session.authenticated()
            {
                return Ok(());
            }
        }
    }

    // Fall back to standard key names
    for key_name in &["id_ed25519", "id_rsa", "id_ecdsa"] {
        let priv_key = ssh_dir.join(key_name);
        if !priv_key.exists() {
            continue;
        }
        let pub_key = ssh_dir.join(format!("{}.pub", key_name));
        let pub_opt: Option<&std::path::Path> = if pub_key.exists() {
            Some(pub_key.as_path())
        } else {
            None
        };
        if session
            .userauth_pubkey_file(user, pub_opt, &priv_key, None)
            .is_ok()
            && session.authenticated()
        {
            return Ok(());
        }
    }

    Err("SSH authentication failed — no valid key found. Check credentials in Settings.".to_string())
}
