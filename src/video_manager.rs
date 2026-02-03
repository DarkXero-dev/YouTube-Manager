use qmetaobject::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;

use crate::ssh_client::SshClient;
use crate::thumbnail::encode_thumbnail_for_qml;
use crate::download_manager::{DownloadState, start_download};

const VPS_HOST: &str = "192.227.180.87";
const VPS_USER: &str = "xero";
const VIDEOS_DIR: &str = "/home/xero/docks/ytdl/downloads/";

#[derive(Default, Clone)]
struct VideoEntry {
    filename: String,
    thumbnail: String,
}

#[derive(QObject)]
pub struct VideoManager {
    base: qt_base_class!(trait QObject),

    // Methods that return state
    get_video_count: qt_method!(fn(&self) -> i32),
    get_status_message: qt_method!(fn(&self) -> QString),
    get_is_loading: qt_method!(fn(&self) -> bool),

    // Download progress methods
    get_download_progress: qt_method!(fn(&self) -> i32),
    get_download_speed: qt_method!(fn(&self) -> QString),
    get_is_downloading: qt_method!(fn(&self) -> bool),
    get_is_paused: qt_method!(fn(&self) -> bool),
    get_download_complete: qt_method!(fn(&self) -> bool),
    get_download_error: qt_method!(fn(&self) -> bool),

    // Action methods
    refresh: qt_method!(fn(&self)),
    get_filename: qt_method!(fn(&self, index: i32) -> QString),
    get_thumbnail: qt_method!(fn(&self, index: i32) -> QString),
    download_video: qt_method!(fn(&self, index: i32, local_path: QString)),
    delete_video: qt_method!(fn(&self, index: i32) -> QString),
    connect_to_vps: qt_method!(fn(&self) -> QString),
    play_video: qt_method!(fn(&self, index: i32) -> QString),

    // Download control methods
    pause_download: qt_method!(fn(&self)),
    resume_download: qt_method!(fn(&self)),
    cancel_download: qt_method!(fn(&self)),
    reset_download_state: qt_method!(fn(&self)),

    // Internal state
    videos: RefCell<Vec<VideoEntry>>,
    ssh_client: RefCell<SshClient>,
    is_loading: RefCell<bool>,
    status_message: RefCell<String>,
    current_download_name: RefCell<String>,

    // Download state (thread-safe)
    download_state: Arc<DownloadState>,
}

impl Default for VideoManager {
    fn default() -> Self {
        Self {
            base: Default::default(),
            get_video_count: Default::default(),
            get_status_message: Default::default(),
            get_is_loading: Default::default(),
            get_download_progress: Default::default(),
            get_download_speed: Default::default(),
            get_is_downloading: Default::default(),
            get_is_paused: Default::default(),
            get_download_complete: Default::default(),
            get_download_error: Default::default(),
            refresh: Default::default(),
            get_filename: Default::default(),
            get_thumbnail: Default::default(),
            download_video: Default::default(),
            delete_video: Default::default(),
            connect_to_vps: Default::default(),
            play_video: Default::default(),
            pause_download: Default::default(),
            resume_download: Default::default(),
            cancel_download: Default::default(),
            reset_download_state: Default::default(),
            videos: RefCell::new(Vec::new()),
            ssh_client: RefCell::new(SshClient::default()),
            is_loading: RefCell::new(false),
            status_message: RefCell::new(String::new()),
            current_download_name: RefCell::new(String::new()),
            download_state: Arc::new(DownloadState::default()),
        }
    }
}

impl VideoManager {
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

    pub fn pause_download(&self) {
        self.download_state.is_paused.store(true, std::sync::atomic::Ordering::SeqCst);
        *self.status_message.borrow_mut() = format!("Paused: {}", self.current_download_name.borrow());
    }

    pub fn resume_download(&self) {
        self.download_state.is_paused.store(false, std::sync::atomic::Ordering::SeqCst);
        *self.status_message.borrow_mut() = format!("Downloading: {}", self.current_download_name.borrow());
    }

    pub fn cancel_download(&self) {
        self.download_state.is_cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
        *self.status_message.borrow_mut() = "Download cancelled".to_string();
    }

    pub fn reset_download_state(&self) {
        self.download_state.reset();
        *self.current_download_name.borrow_mut() = String::new();
    }

    pub fn connect_to_vps(&self) -> QString {
        *self.is_loading.borrow_mut() = true;
        *self.status_message.borrow_mut() = "Connecting to VPS...".to_string();

        let mut client = self.ssh_client.borrow_mut();
        let connect_result = client.connect();
        drop(client);

        match connect_result {
            Ok(_) => {
                *self.status_message.borrow_mut() = "Connected! Loading videos...".to_string();
                self.refresh();
                QString::from("success")
            }
            Err(e) => {
                let msg = format!("Connection failed: {}", e);
                *self.status_message.borrow_mut() = msg.clone();
                *self.is_loading.borrow_mut() = false;
                QString::from(msg)
            }
        }
    }

    pub fn refresh(&self) {
        *self.is_loading.borrow_mut() = true;
        *self.status_message.borrow_mut() = "Loading video list...".to_string();

        let client = self.ssh_client.borrow();
        if !client.is_connected() {
            drop(client);
            *self.status_message.borrow_mut() = "Not connected to VPS".to_string();
            *self.is_loading.borrow_mut() = false;
            return;
        }

        let videos_result = client.list_videos();

        match videos_result {
            Ok(filenames) => {
                let mut entries = Vec::new();
                let total = filenames.len();

                for (i, filename) in filenames.into_iter().enumerate() {
                    *self.status_message.borrow_mut() = format!("Loading thumbnail {}/{}...", i + 1, total);

                    let thumbnail = match client.get_thumbnail(&filename) {
                        Ok(data) => encode_thumbnail_for_qml(&data),
                        Err(_) => String::new(),
                    };

                    entries.push(VideoEntry {
                        filename,
                        thumbnail,
                    });
                }

                drop(client);

                let count = entries.len();
                *self.videos.borrow_mut() = entries;
                *self.status_message.borrow_mut() = format!("{} videos loaded", count);
            }
            Err(e) => {
                drop(client);
                *self.status_message.borrow_mut() = format!("Failed to list videos: {}", e);
            }
        }

        *self.is_loading.borrow_mut() = false;
    }

    pub fn get_filename(&self, index: i32) -> QString {
        let videos = self.videos.borrow();
        if index >= 0 && (index as usize) < videos.len() {
            QString::from(videos[index as usize].filename.clone())
        } else {
            QString::default()
        }
    }

    pub fn get_thumbnail(&self, index: i32) -> QString {
        let videos = self.videos.borrow();
        if index >= 0 && (index as usize) < videos.len() {
            QString::from(videos[index as usize].thumbnail.clone())
        } else {
            QString::default()
        }
    }

    pub fn download_video(&self, index: i32, local_path: QString) {
        let videos = self.videos.borrow();
        if index < 0 || (index as usize) >= videos.len() {
            *self.status_message.borrow_mut() = "Invalid video index".to_string();
            return;
        }

        let filename = videos[index as usize].filename.clone();
        drop(videos);

        let local_path_str = local_path.to_string();
        let full_path = format!("{}/{}", local_path_str.trim_end_matches('/'), filename);
        let remote_path = format!("{}{}", VIDEOS_DIR, filename);

        *self.current_download_name.borrow_mut() = filename.clone();
        *self.status_message.borrow_mut() = format!("Downloading: {}", filename);

        start_download(
            self.download_state.clone(),
            VPS_HOST.to_string(),
            VPS_USER.to_string(),
            remote_path,
            full_path,
        );
    }

    pub fn delete_video(&self, index: i32) -> QString {
        let videos = self.videos.borrow();
        if index < 0 || (index as usize) >= videos.len() {
            return QString::from("Invalid video index");
        }

        let filename = videos[index as usize].filename.clone();
        drop(videos);

        *self.is_loading.borrow_mut() = true;
        *self.status_message.borrow_mut() = format!("Deleting {}...", filename);

        let client = self.ssh_client.borrow();
        let delete_result = client.delete_video(&filename);
        drop(client);

        let result = match delete_result {
            Ok(_) => {
                *self.status_message.borrow_mut() = format!("Deleted: {}", filename);
                self.refresh();
                QString::from(format!("Deleted {}", filename))
            }
            Err(e) => {
                let msg = format!("Delete failed: {}", e);
                *self.status_message.borrow_mut() = msg.clone();
                *self.is_loading.borrow_mut() = false;
                QString::from(msg)
            }
        };

        result
    }

    pub fn play_video(&self, index: i32) -> QString {
        let videos = self.videos.borrow();
        if index < 0 || (index as usize) >= videos.len() {
            return QString::from("Invalid video index");
        }

        let filename = videos[index as usize].filename.clone();
        drop(videos);

        let sftp_url = format!(
            "sftp://{}@{}{}/{}",
            VPS_USER, VPS_HOST, VIDEOS_DIR.trim_end_matches('/'), filename
        );

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
                let msg = format!("Failed to launch player: {}", e);
                *self.status_message.borrow_mut() = msg.clone();
                QString::from(msg)
            }
        }
    }
}
