use qmetaobject::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use crate::config::Config;
use crate::ssh_client::SshClient;
use crate::thumbnail::encode_thumbnail_for_qml;
use crate::download_manager::{DownloadState, start_download};

#[derive(Default, Clone)]
struct VideoEntry {
    filename: String,
    thumbnail: String,
}

#[derive(QObject)]
pub struct VideoManager {
    base: qt_base_class!(trait QObject),

    // State getters
    get_video_count:     qt_method!(fn(&self) -> i32),
    get_status_message:  qt_method!(fn(&self) -> QString),
    get_is_loading:      qt_method!(fn(&self) -> bool),

    // Download progress
    get_download_progress:  qt_method!(fn(&self) -> i32),
    get_download_speed:     qt_method!(fn(&self) -> QString),
    get_is_downloading:     qt_method!(fn(&self) -> bool),
    get_is_paused:          qt_method!(fn(&self) -> bool),
    get_download_complete:  qt_method!(fn(&self) -> bool),
    get_download_error:     qt_method!(fn(&self) -> bool),

    // Video actions
    refresh:        qt_method!(fn(&self)),
    get_filename:   qt_method!(fn(&self, index: i32) -> QString),
    get_thumbnail:  qt_method!(fn(&self, index: i32) -> QString),
    download_video: qt_method!(fn(&self, index: i32, local_path: QString)),
    delete_video:   qt_method!(fn(&self, index: i32) -> QString),
    connect_to_vps: qt_method!(fn(&self) -> QString),
    play_video:     qt_method!(fn(&self, index: i32) -> QString),

    // Download controls
    pause_download:       qt_method!(fn(&self)),
    resume_download:      qt_method!(fn(&self)),
    cancel_download:      qt_method!(fn(&self)),
    reset_download_state: qt_method!(fn(&self)),

    // Batch actions
    batch_delete_videos: qt_method!(fn(&self, indices_csv: QString) -> QString),

    // Thumbnail async polling
    poll_thumbnails: qt_method!(fn(&self) -> i32),

    // Config / credentials
    has_valid_key:         qt_method!(fn(&self) -> bool),
    is_connected:          qt_method!(fn(&self) -> bool),
    get_config_host:       qt_method!(fn(&self) -> QString),
    get_config_user:       qt_method!(fn(&self) -> QString),
    get_config_videos_dir: qt_method!(fn(&self) -> QString),
    save_videos_dir:       qt_method!(fn(&self, dir: QString)),
    // Lists subdirectories on the VPS at `path`. Returns "\n"-separated names,
    // or "ERROR:<message>" if the listing fails.
    list_remote_dirs:      qt_method!(fn(&self, path: QString) -> QString),
    // Generates SSH key, installs it on VPS via password, saves config.
    // Returns "success" or an error string.
    setup_credentials:     qt_method!(fn(&self, host: QString, user: QString, password: QString) -> QString),

    // Error / crash reporting
    has_error:      qt_method!(fn(&self) -> bool),
    get_last_error: qt_method!(fn(&self) -> QString),
    get_crash_log:  qt_method!(fn(&self) -> QString),

    // Internal state
    videos:                  RefCell<Vec<VideoEntry>>,
    ssh_client:              RefCell<SshClient>,
    is_loading:              RefCell<bool>,
    status_message:          RefCell<String>,
    current_download_name:   RefCell<String>,
    config:                  RefCell<Config>,
    thumbnail_rx:            RefCell<Option<Receiver<(usize, String)>>>,
    last_error:              RefCell<Option<String>>,

    // Download state (thread-safe)
    download_state: Arc<DownloadState>,
}

impl Default for VideoManager {
    fn default() -> Self {
        Self {
            base:                    Default::default(),
            get_video_count:         Default::default(),
            get_status_message:      Default::default(),
            get_is_loading:          Default::default(),
            get_download_progress:   Default::default(),
            get_download_speed:      Default::default(),
            get_is_downloading:      Default::default(),
            get_is_paused:           Default::default(),
            get_download_complete:   Default::default(),
            get_download_error:      Default::default(),
            refresh:                 Default::default(),
            get_filename:            Default::default(),
            get_thumbnail:           Default::default(),
            download_video:          Default::default(),
            delete_video:            Default::default(),
            connect_to_vps:          Default::default(),
            play_video:              Default::default(),
            pause_download:          Default::default(),
            resume_download:         Default::default(),
            cancel_download:         Default::default(),
            reset_download_state:    Default::default(),
            batch_delete_videos:     Default::default(),
            poll_thumbnails:         Default::default(),
            has_valid_key:           Default::default(),
            is_connected:            Default::default(),
            get_config_host:         Default::default(),
            get_config_user:         Default::default(),
            get_config_videos_dir:   Default::default(),
            save_videos_dir:         Default::default(),
            list_remote_dirs:        Default::default(),
            setup_credentials:       Default::default(),
            has_error:               Default::default(),
            get_last_error:          Default::default(),
            get_crash_log:           Default::default(),
            videos:                  RefCell::new(Vec::new()),
            ssh_client:              RefCell::new(SshClient::default()),
            is_loading:              RefCell::new(false),
            status_message:          RefCell::new(String::new()),
            current_download_name:   RefCell::new(String::new()),
            config:                  RefCell::new(Config::load()),
            thumbnail_rx:            RefCell::new(None),
            last_error:              RefCell::new(None),
            download_state:          Arc::new(DownloadState::default()),
        }
    }
}

impl VideoManager {
    // ── State getters ────────────────────────────────────────────────────────

    pub fn get_video_count(&self) -> i32 {
        self.videos.borrow().len() as i32
    }

    pub fn get_status_message(&self) -> QString {
        QString::from(self.status_message.borrow().clone())
    }

    pub fn get_is_loading(&self) -> bool {
        *self.is_loading.borrow()
    }

    pub fn get_download_progress(&self) -> i32 {
        self.download_state.progress.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn get_download_speed(&self) -> QString {
        QString::from(self.download_state.get_speed_string())
    }

    pub fn get_is_downloading(&self) -> bool {
        self.download_state.is_active.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn get_is_paused(&self) -> bool {
        self.download_state.is_paused.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn get_download_complete(&self) -> bool {
        self.download_state.is_complete.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn get_download_error(&self) -> bool {
        self.download_state.has_error.load(std::sync::atomic::Ordering::SeqCst)
    }

    // ── Download controls ────────────────────────────────────────────────────

    pub fn pause_download(&self) {
        self.download_state.is_paused.store(true, std::sync::atomic::Ordering::SeqCst);
        let name = self.current_download_name.borrow().clone();
        *self.status_message.borrow_mut() = format!("Paused: {}", name);
    }

    pub fn resume_download(&self) {
        self.download_state.is_paused.store(false, std::sync::atomic::Ordering::SeqCst);
        let name = self.current_download_name.borrow().clone();
        *self.status_message.borrow_mut() = format!("Downloading: {}", name);
    }

    pub fn cancel_download(&self) {
        self.download_state.is_cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
        *self.status_message.borrow_mut() = "Download cancelled".to_string();
    }

    pub fn reset_download_state(&self) {
        self.download_state.reset();
        *self.current_download_name.borrow_mut() = String::new();
    }

    // ── Config / credentials ─────────────────────────────────────────────────

    pub fn has_valid_key(&self) -> bool {
        self.config.borrow().has_valid_key()
    }

    pub fn is_connected(&self) -> bool {
        self.ssh_client.borrow().is_connected()
    }

    pub fn get_config_host(&self) -> QString {
        QString::from(self.config.borrow().host.clone())
    }

    pub fn get_config_user(&self) -> QString {
        QString::from(self.config.borrow().user.clone())
    }

    pub fn get_config_videos_dir(&self) -> QString {
        QString::from(self.config.borrow().videos_dir.clone())
    }

    pub fn save_videos_dir(&self, dir: QString) {
        let mut cfg = self.config.borrow_mut();
        let mut d = dir.to_string();
        if !d.ends_with('/') { d.push('/'); }
        cfg.videos_dir = d;
        if let Err(e) = cfg.save() {
            eprintln!("Failed to save videos_dir: {}", e);
        }
    }

    /// Lists subdirectories on the VPS at `path`.
    /// Returns newline-separated directory names, or "ERROR:<message>" on failure.
    pub fn list_remote_dirs(&self, path: QString) -> QString {
        let path_str = path.to_string();
        let client = self.ssh_client.borrow();
        if !client.is_connected() {
            return QString::from("ERROR:Not connected — connect first.");
        }
        match client.list_dirs(&path_str) {
            Ok(dirs) => QString::from(dirs.join("\n")),
            Err(e)   => QString::from(format!("ERROR:{}", e)),
        }
    }

    /// Generates an SSH key pair (if needed), connects to the VPS with the
    /// supplied password, installs the public key in authorized_keys, then
    /// saves host/user/key_path to the config file.
    /// Returns "success" or a human-readable error string.
    pub fn setup_credentials(&self, host: QString, user: QString, password: QString) -> QString {
        let host_str = host.to_string();
        let user_str = user.to_string();
        let pass_str = password.to_string();

        if host_str.is_empty() || user_str.is_empty() || pass_str.is_empty() {
            return QString::from("Please fill in all fields.");
        }

        let port = self.config.borrow().port;
        let videos_dir = self.config.borrow().videos_dir.clone();

        match crate::ssh_client::setup_key_auth(&host_str, port, &user_str, &pass_str) {
            Ok(key_path) => {
                let mut cfg = self.config.borrow_mut();
                cfg.host = host_str;
                cfg.user = user_str;
                cfg.key_path = Some(key_path.to_string_lossy().to_string());
                cfg.videos_dir = videos_dir;
                if let Err(e) = cfg.save() {
                    return QString::from(format!("Key installed but config save failed: {}", e));
                }
                QString::from("success")
            }
            Err(e) => {
                self.set_error(e.clone());
                QString::from(e)
            }
        }
    }

    // ── Error / crash reporting ──────────────────────────────────────────────

    pub fn has_error(&self) -> bool {
        self.last_error.borrow().is_some()
    }

    pub fn get_last_error(&self) -> QString {
        let msg = self.last_error.borrow_mut().take().unwrap_or_default();
        QString::from(msg)
    }

    pub fn get_crash_log(&self) -> QString {
        let path = crate::config::crash_log_path();
        if !path.exists() {
            return QString::default();
        }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::remove_file(&path);
        QString::from(content)
    }

    fn set_error(&self, msg: String) {
        *self.last_error.borrow_mut() = Some(msg);
    }

    // ── Connection ───────────────────────────────────────────────────────────

    pub fn connect_to_vps(&self) -> QString {
        *self.is_loading.borrow_mut() = true;
        *self.status_message.borrow_mut() = "Connecting...".to_string();

        let config = self.config.borrow().clone();
        let connect_result = self.ssh_client.borrow_mut().connect(&config);

        match connect_result {
            Ok(_) => {
                *self.status_message.borrow_mut() = "Connected".to_string();
                self.refresh();
                QString::from("success")
            }
            Err(e) => {
                let msg = format!("Connection failed: {}", e);
                *self.status_message.borrow_mut() = msg.clone();
                *self.is_loading.borrow_mut() = false;
                self.set_error(msg.clone());
                QString::from(msg)
            }
        }
    }

    // ── Video list (fast — thumbnails load asynchronously) ───────────────────

    pub fn refresh(&self) {
        *self.is_loading.borrow_mut() = true;
        *self.status_message.borrow_mut() = "Loading video list...".to_string();

        // Cancel any running thumbnail thread by dropping the old receiver
        *self.thumbnail_rx.borrow_mut() = None;

        let client = self.ssh_client.borrow();
        if !client.is_connected() {
            drop(client);
            *self.status_message.borrow_mut() = "Not connected. Please reconnect.".to_string();
            *self.is_loading.borrow_mut() = false;
            return;
        }

        match client.list_videos() {
            Ok(filenames) => {
                drop(client);
                let count = filenames.len();

                // Populate filenames immediately; thumbnails start empty
                *self.videos.borrow_mut() = filenames
                    .iter()
                    .map(|f| VideoEntry { filename: f.clone(), thumbnail: String::new() })
                    .collect();

                *self.status_message.borrow_mut() = format!("{} videos loaded", count);
                *self.is_loading.borrow_mut() = false;

                // Start background thumbnail loading
                if !filenames.is_empty() {
                    let config = self.config.borrow().clone();
                    let (tx, rx) = mpsc::channel();
                    *self.thumbnail_rx.borrow_mut() = Some(rx);
                    load_thumbnails_background(config, filenames, tx);
                }
            }
            Err(e) => {
                drop(client);
                let msg = format!("Failed to list videos: {}", e);
                *self.status_message.borrow_mut() = msg.clone();
                *self.is_loading.borrow_mut() = false;
                self.set_error(msg);
            }
        }
    }

    // ── Thumbnail polling (called by QML timer) ───────────────────────────────

    pub fn poll_thumbnails(&self) -> i32 {
        let mut count = 0i32;
        let rx_ref = self.thumbnail_rx.borrow();
        if let Some(ref rx) = *rx_ref {
            loop {
                match rx.try_recv() {
                    Ok((index, thumbnail)) => {
                        let mut videos = self.videos.borrow_mut();
                        if index < videos.len() {
                            videos[index].thumbnail = thumbnail;
                            count += 1;
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
        }
        count
    }

    // ── Per-video accessors ──────────────────────────────────────────────────

    pub fn get_filename(&self, index: i32) -> QString {
        let videos = self.videos.borrow();
        match videos.get(index as usize) {
            Some(v) => QString::from(v.filename.clone()),
            None => QString::default(),
        }
    }

    pub fn get_thumbnail(&self, index: i32) -> QString {
        let videos = self.videos.borrow();
        match videos.get(index as usize) {
            Some(v) => QString::from(v.thumbnail.clone()),
            None => QString::default(),
        }
    }

    // ── Download ─────────────────────────────────────────────────────────────

    pub fn download_video(&self, index: i32, local_path: QString) {
        let videos = self.videos.borrow();
        let entry = match videos.get(index as usize) {
            Some(e) => e.clone(),
            None => {
                *self.status_message.borrow_mut() = "Invalid video index".to_string();
                return;
            }
        };
        drop(videos);

        let config = self.config.borrow().clone();
        let local_path_str = local_path.to_string();
        let full_path = format!("{}/{}", local_path_str.trim_end_matches('/'), entry.filename);
        let remote_path = format!("{}{}", config.videos_dir, entry.filename);

        *self.current_download_name.borrow_mut() = entry.filename.clone();
        *self.status_message.borrow_mut() = format!("Downloading: {}", entry.filename);

        start_download(
            self.download_state.clone(),
            config.host,
            config.user,
            remote_path,
            full_path,
            config.key_path,
        );
    }

    // ── Delete (single) ──────────────────────────────────────────────────────

    pub fn delete_video(&self, index: i32) -> QString {
        let videos = self.videos.borrow();
        let filename = match videos.get(index as usize) {
            Some(v) => v.filename.clone(),
            None => return QString::from("Invalid video index"),
        };
        drop(videos);

        *self.is_loading.borrow_mut() = true;
        *self.status_message.borrow_mut() = format!("Deleting {}...", filename);

        let result = self.ssh_client.borrow().delete_video(&filename);
        let out = match result {
            Ok(_) => {
                *self.status_message.borrow_mut() = format!("Deleted: {}", filename);
                self.refresh();
                format!("Deleted {}", filename)
            }
            Err(e) => {
                let msg = format!("Delete failed: {}", e);
                *self.status_message.borrow_mut() = msg.clone();
                *self.is_loading.borrow_mut() = false;
                self.set_error(msg.clone());
                msg
            }
        };
        QString::from(out)
    }

    // ── Batch delete ─────────────────────────────────────────────────────────

    pub fn batch_delete_videos(&self, indices_csv: QString) -> QString {
        let indices_str = indices_csv.to_string();
        if indices_str.is_empty() {
            return QString::from("No videos selected");
        }

        let indices: Vec<usize> = indices_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if indices.is_empty() {
            return QString::from("No valid indices");
        }

        // Collect filenames before any deletion (avoids index-shift bugs)
        let filenames: Vec<String> = {
            let videos = self.videos.borrow();
            indices
                .iter()
                .filter_map(|&i| videos.get(i).map(|v| v.filename.clone()))
                .collect()
        };

        *self.is_loading.borrow_mut() = true;
        *self.status_message.borrow_mut() = format!("Deleting {} video(s)...", filenames.len());

        let client = self.ssh_client.borrow();
        let mut success = 0usize;
        let mut errors: Vec<String> = Vec::new();

        for filename in &filenames {
            match client.delete_video(filename) {
                Ok(_) => success += 1,
                Err(e) => errors.push(format!("{}: {}", filename, e)),
            }
        }
        drop(client);

        // Single refresh after all deletions
        self.refresh();

        let result = if errors.is_empty() {
            format!("Deleted {} video(s)", success)
        } else {
            format!(
                "Deleted {}/{} video(s). Errors: {}",
                success,
                filenames.len(),
                errors.join("; ")
            )
        };

        *self.status_message.borrow_mut() = result.clone();
        if !errors.is_empty() {
            self.set_error(result.clone());
        }
        QString::from(result)
    }

    // ── Play ─────────────────────────────────────────────────────────────────

    pub fn play_video(&self, index: i32) -> QString {
        let videos = self.videos.borrow();
        let filename = match videos.get(index as usize) {
            Some(v) => v.filename.clone(),
            None => return QString::from("Invalid video index"),
        };
        drop(videos);

        let config = self.config.borrow();
        let sftp_url = format!(
            "sftp://{}@{}{}/{}",
            config.user,
            config.host,
            config.videos_dir.trim_end_matches('/'),
            filename,
        );
        drop(config);

        *self.status_message.borrow_mut() = format!("Playing: {}", filename);

        match std::process::Command::new("mpv")
            .arg("--force-window=yes")
            .arg("--autofit=1530x860")
            .arg("--title=Xero Video Player")
            .arg(&sftp_url)
            .spawn()
        {
            Ok(_) => QString::from("Playing"),
            Err(e) => {
                let msg = format!("Failed to launch mpv: {}", e);
                *self.status_message.borrow_mut() = msg.clone();
                self.set_error(msg.clone());
                QString::from(msg)
            }
        }
    }
}

// ── Background thumbnail loader ───────────────────────────────────────────────
//
// Creates its own SSH connection so the main thread is never blocked by
// ffmpeg thumbnail generation.

fn load_thumbnails_background(
    config: Config,
    filenames: Vec<String>,
    sender: mpsc::Sender<(usize, String)>,
) {
    std::thread::spawn(move || {
        let mut client = SshClient::new();
        if client.connect(&config).is_err() {
            return; // Can't load thumbnails without a connection
        }

        for (i, filename) in filenames.iter().enumerate() {
            if let Ok(data) = client.get_thumbnail(filename) {
                let encoded = encode_thumbnail_for_qml(&data);
                if sender.send((i, encoded)).is_err() {
                    // Receiver was dropped (new refresh started) — stop here
                    break;
                }
            }
            // Failed thumbnails are skipped silently; the placeholder stays
        }
    });
}
