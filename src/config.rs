use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: Option<String>,
    pub videos_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: "".to_string(),
            port: 22,
            user: "".to_string(),
            key_path: None,
            videos_dir: "".to_string(),
        }
    }
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xero-yt-manager")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config")
}

pub fn crash_log_path() -> PathBuf {
    config_dir().join("crash.log")
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        let mut cfg = Config::default();

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return cfg,
        };

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let v = value.trim();
                match key.trim() {
                    "host" => cfg.host = v.to_string(),
                    "port" => cfg.port = v.parse().unwrap_or(22),
                    "user" => cfg.user = v.to_string(),
                    "key_path" => cfg.key_path = if v.is_empty() { None } else { Some(v.to_string()) },
                    "videos_dir" => cfg.videos_dir = v.to_string(),
                    _ => {}
                }
            }
        }

        cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let content = format!(
            "host={}\nport={}\nuser={}\nkey_path={}\nvideos_dir={}\n",
            self.host,
            self.port,
            self.user,
            self.key_path.as_deref().unwrap_or(""),
            self.videos_dir,
        );

        std::fs::write(config_path(), content).map_err(|e| e.to_string())
    }

    /// Returns true if a usable SSH private key exists.
    pub fn has_valid_key(&self) -> bool {
        if let Some(ref path) = self.key_path {
            return std::path::Path::new(path).exists();
        }
        let ssh_dir = match dirs::home_dir() {
            Some(h) => h.join(".ssh"),
            None => return false,
        };
        for key in &["id_ed25519", "id_rsa", "id_ecdsa"] {
            if ssh_dir.join(key).exists() {
                return true;
            }
        }
        false
    }
}
