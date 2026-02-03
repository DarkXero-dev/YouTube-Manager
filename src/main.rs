mod ssh_client;
mod thumbnail;
mod video_manager;
mod download_manager;

use qmetaobject::prelude::*;
use video_manager::VideoManager;

fn find_qml_path() -> String {
    // Check local development path first
    let local_path = std::env::current_dir()
        .map(|p| p.join("qml/main.qml"))
        .ok();

    if let Some(path) = local_path {
        if path.exists() {
            return path.to_string_lossy().to_string();
        }
    }

    // Check installed system path
    let system_path = std::path::Path::new("/usr/share/ytm/qml/main.qml");
    if system_path.exists() {
        return system_path.to_string_lossy().to_string();
    }

    // Fallback to relative path
    "qml/main.qml".to_string()
}

fn main() {
    qml_register_type::<VideoManager>(
        cstr::cstr!("VideoManager"),
        1,
        0,
        cstr::cstr!("VideoManager"),
    );

    let mut engine = QmlEngine::new();

    let qml_path = find_qml_path();
    engine.load_file(qml_path.into());

    engine.exec();
}
