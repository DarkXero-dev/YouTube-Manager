use ssh2::Session;
use std::io::Read;
use std::net::TcpStream;

const VPS_HOST: &str = "192.227.180.87";
const VPS_PORT: u16 = 22;
const VPS_USER: &str = "xero";
const VIDEOS_DIR: &str = "/home/xero/docks/ytdl/downloads/";

pub struct SshClient {
    session: Option<Session>,
}

impl SshClient {
    pub fn new() -> Self {
        SshClient { session: None }
    }

    pub fn connect(&mut self) -> Result<(), String> {
        let tcp = TcpStream::connect(format!("{}:{}", VPS_HOST, VPS_PORT))
            .map_err(|e| format!("Failed to connect to VPS: {}", e))?;

        let mut session = Session::new().map_err(|e| format!("Failed to create session: {}", e))?;
        session.set_tcp_stream(tcp);
        session.handshake().map_err(|e| format!("SSH handshake failed: {}", e))?;

        // Try SSH keys from ~/.ssh/
        let ssh_dir = dirs::home_dir()
            .ok_or_else(|| "Could not find home directory".to_string())?
            .join(".ssh");

        let key_names = ["id_ed25519", "id_rsa", "id_ecdsa"];
        let mut authenticated = false;

        for key_name in &key_names {
            let private_key = ssh_dir.join(key_name);
            if private_key.exists() {
                let public_key = ssh_dir.join(format!("{}.pub", key_name));
                let pub_key_opt = if public_key.exists() {
                    Some(public_key.as_path())
                } else {
                    None
                };

                match session.userauth_pubkey_file(VPS_USER, pub_key_opt, &private_key, None) {
                    Ok(_) => {
                        authenticated = true;
                        break;
                    }
                    Err(_) => continue,
                }
            }
        }

        if !authenticated {
            return Err("No valid SSH key found in ~/.ssh/".to_string());
        }

        if !session.authenticated() {
            return Err("SSH authentication failed".to_string());
        }

        self.session = Some(session);
        Ok(())
    }

    pub fn list_videos(&self) -> Result<Vec<String>, String> {
        let session = self.session.as_ref().ok_or("Not connected")?;

        let sftp = session.sftp().map_err(|e| format!("SFTP failed: {}", e))?;
        let entries = sftp
            .readdir(std::path::Path::new(VIDEOS_DIR))
            .map_err(|e| format!("Failed to list directory: {}", e))?;

        let video_extensions = ["mp4", "mkv", "webm", "avi", "mov", "flv"];
        let mut videos: Vec<String> = entries
            .into_iter()
            .filter_map(|(path, _stat)| {
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

        // Escape single quotes in filename for shell
        let escaped_filename = filename.replace("'", "'\\''");
        let video_path = format!("{}{}", VIDEOS_DIR, escaped_filename);
        let cmd = format!(
            "ffmpeg -y -i '{}' -ss 00:00:01 -vframes 1 -vf scale=280:-1 -f image2pipe -vcodec mjpeg - 2>/dev/null",
            video_path
        );

        let mut channel = session
            .channel_session()
            .map_err(|e| format!("Channel failed: {}", e))?;

        channel.exec(&cmd).map_err(|e| format!("Exec failed: {}", e))?;

        let mut thumbnail_data = Vec::new();
        channel
            .read_to_end(&mut thumbnail_data)
            .map_err(|e| format!("Read failed: {}", e))?;

        // Properly close the channel
        let _ = channel.send_eof();
        let _ = channel.wait_eof();
        let _ = channel.close();
        let _ = channel.wait_close();

        if thumbnail_data.is_empty() {
            return Err("No thumbnail generated".to_string());
        }

        Ok(thumbnail_data)
    }

    pub fn delete_video(&self, filename: &str) -> Result<(), String> {
        let session = self.session.as_ref().ok_or("Not connected")?;

        let sftp = session.sftp().map_err(|e| format!("SFTP failed: {}", e))?;
        let remote_path = format!("{}{}", VIDEOS_DIR, filename);

        sftp.unlink(std::path::Path::new(&remote_path))
            .map_err(|e| format!("Failed to delete file: {}", e))?;

        Ok(())
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
