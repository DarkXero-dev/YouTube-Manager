use std::sync::atomic::{AtomicBool, AtomicU64, AtomicI32, Ordering};
use std::sync::Arc;
use std::thread;
use std::io::{Read, Write};

pub struct DownloadState {
    pub progress: AtomicI32,      // 0-100
    pub speed_bytes: AtomicU64,   // bytes per second
    pub downloaded: AtomicU64,    // bytes downloaded
    pub total_size: AtomicU64,    // total bytes
    pub is_active: AtomicBool,    // download in progress
    pub is_paused: AtomicBool,    // paused state
    pub is_cancelled: AtomicBool, // cancelled
    pub is_complete: AtomicBool,  // finished successfully
    pub has_error: AtomicBool,    // finished with error
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            progress: AtomicI32::new(0),
            speed_bytes: AtomicU64::new(0),
            downloaded: AtomicU64::new(0),
            total_size: AtomicU64::new(0),
            is_active: AtomicBool::new(false),
            is_paused: AtomicBool::new(false),
            is_cancelled: AtomicBool::new(false),
            is_complete: AtomicBool::new(false),
            has_error: AtomicBool::new(false),
        }
    }
}

impl DownloadState {
    pub fn reset(&self) {
        self.progress.store(0, Ordering::SeqCst);
        self.speed_bytes.store(0, Ordering::SeqCst);
        self.downloaded.store(0, Ordering::SeqCst);
        self.total_size.store(0, Ordering::SeqCst);
        self.is_active.store(false, Ordering::SeqCst);
        self.is_paused.store(false, Ordering::SeqCst);
        self.is_cancelled.store(false, Ordering::SeqCst);
        self.is_complete.store(false, Ordering::SeqCst);
        self.has_error.store(false, Ordering::SeqCst);
    }

    pub fn get_speed_string(&self) -> String {
        let bytes = self.speed_bytes.load(Ordering::SeqCst);
        if bytes >= 1_000_000 {
            format!("{:.1} MB/s", bytes as f64 / 1_000_000.0)
        } else if bytes >= 1_000 {
            format!("{:.0} KB/s", bytes as f64 / 1_000.0)
        } else {
            format!("{} B/s", bytes)
        }
    }
}

pub fn start_download(
    state: Arc<DownloadState>,
    host: String,
    user: String,
    remote_path: String,
    local_path: String,
    key_path: Option<String>,
) {
    state.reset();
    state.is_active.store(true, Ordering::SeqCst);

    thread::spawn(move || {
        let result = do_download(&state, &host, &user, &remote_path, &local_path, key_path.as_deref());

        state.is_active.store(false, Ordering::SeqCst);

        match result {
            Ok(_) => {
                state.is_complete.store(true, Ordering::SeqCst);
                state.progress.store(100, Ordering::SeqCst);
            }
            Err(_) => {
                if !state.is_cancelled.load(Ordering::SeqCst) {
                    state.has_error.store(true, Ordering::SeqCst);
                }
            }
        }
    });
}

fn do_download(
    state: &Arc<DownloadState>,
    host: &str,
    user: &str,
    remote_path: &str,
    local_path: &str,
    key_path: Option<&str>,
) -> Result<(), String> {
    use std::net::ToSocketAddrs;
    use std::time::Duration;

    let addr_str = format!("{}:22", host);
    let addr = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("Address error: {}", e))?
        .next()
        .ok_or("No address")?;

    let tcp = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|e| format!("Connect failed: {}", e))?;
    let _ = tcp.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = tcp.set_write_timeout(Some(Duration::from_secs(30)));

    let mut session = ssh2::Session::new()
        .map_err(|e| format!("Session failed: {}", e))?;
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|e| format!("Handshake failed: {}", e))?;

    crate::ssh_client::authenticate_session(&mut session, user, key_path)
        .map_err(|e| format!("Auth failed: {}", e))?;

    // Open SFTP
    let sftp = session
        .sftp()
        .map_err(|e| format!("SFTP failed: {}", e))?;

    // Get file size
    let file_stat = sftp
        .stat(std::path::Path::new(remote_path))
        .map_err(|e| format!("Stat failed: {}", e))?;
    let total_size = file_stat.size.unwrap_or(0);
    state.total_size.store(total_size, Ordering::SeqCst);

    // Open remote file
    let mut remote_file = sftp
        .open(std::path::Path::new(remote_path))
        .map_err(|e| format!("Open failed: {}", e))?;

    // Create local file
    let mut local_file = std::fs::File::create(local_path)
        .map_err(|e| format!("Create failed: {}", e))?;

    let mut buffer = [0u8; 65536];
    let mut bytes_downloaded: u64 = 0;
    let mut last_speed_update = std::time::Instant::now();
    let mut bytes_since_last_update: u64 = 0;

    loop {
        if state.is_cancelled.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(local_path);
            return Err("Cancelled".to_string());
        }

        while state.is_paused.load(Ordering::SeqCst) {
            if state.is_cancelled.load(Ordering::SeqCst) {
                let _ = std::fs::remove_file(local_path);
                return Err("Cancelled".to_string());
            }
            thread::sleep(Duration::from_millis(100));
        }

        let bytes_read = remote_file
            .read(&mut buffer)
            .map_err(|e| format!("Read error: {}", e))?;

        if bytes_read == 0 {
            break;
        }

        local_file
            .write_all(&buffer[..bytes_read])
            .map_err(|e| format!("Write error: {}", e))?;

        bytes_downloaded += bytes_read as u64;
        bytes_since_last_update += bytes_read as u64;
        state.downloaded.store(bytes_downloaded, Ordering::SeqCst);

        if total_size > 0 {
            let progress = ((bytes_downloaded as f64 / total_size as f64) * 100.0) as i32;
            state.progress.store(progress, Ordering::SeqCst);
        }

        let now = std::time::Instant::now();
        let elapsed = now.duration_since(last_speed_update).as_secs_f64();
        if elapsed >= 0.5 {
            let speed = (bytes_since_last_update as f64 / elapsed) as u64;
            state.speed_bytes.store(speed, Ordering::SeqCst);
            bytes_since_last_update = 0;
            last_speed_update = now;
        }
    }

    Ok(())
}
