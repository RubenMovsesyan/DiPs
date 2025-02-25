// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use dips;
use dips::DiPsProperties;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command(rename_all = "snake_case")]
fn run_dips(input_path: &str, output_path: &str) {
    dips::init();

    let mut properties = DiPsProperties::new()
        .video_path(input_path)
        .output_path(output_path)
        .build();

    dips::perform_dips(&mut properties);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![run_dips])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
