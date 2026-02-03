use base64::{engine::general_purpose::STANDARD, Engine};

/// Encode raw thumbnail bytes to a base64 data URI for QML
pub fn encode_thumbnail_for_qml(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }

    let base64_data = STANDARD.encode(data);
    format!("data:image/jpeg;base64,{}", base64_data)
}
