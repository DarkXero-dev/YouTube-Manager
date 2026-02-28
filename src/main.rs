mod config;
mod ssh_client;
mod thumbnail;
mod video_manager;
mod download_manager;

use qmetaobject::prelude::*;
use video_manager::VideoManager;

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let message = format!("{}", info);
        eprintln!("PANIC: {}", message);

        if let Some(log_path) = dirs::config_dir()
            .map(|d| d.join("xero-yt-manager").join("crash.log"))
        {
            let _ = std::fs::create_dir_all(log_path.parent().unwrap());
            let _ = std::fs::write(&log_path, &message);
        }
    }));
}

fn find_qml_path() -> String {
    let local_path = std::env::current_dir()
        .map(|p| p.join("qml/main.qml"))
        .ok();

    if let Some(path) = local_path {
        if path.exists() {
            return path.to_string_lossy().to_string();
        }
    }

    let system_path = std::path::Path::new("/usr/share/ytm/qml/main.qml");
    if system_path.exists() {
        return system_path.to_string_lossy().to_string();
    }

    "qml/main.qml".to_string()
}

fn main() {
    setup_panic_hook();

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
