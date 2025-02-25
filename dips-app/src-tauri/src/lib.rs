// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use dips;
use dips::DiPsProperties;

#[tauri::command(rename_all = "snake_case")]
fn run_dips(input_path: &str, output_path: &str) {
    dips::init_frame_extractor();

    let mut properties = DiPsProperties::new()
        .video_path(input_path)
        .output_path(output_path)
        .build();

    dips::perform_dips(&mut properties);
}

#[tauri::command(rename_all = "snake_case")]
fn get_thumbnail(input_path: &str, cache_path: &str) {
    dips::init_thumbnail_extractor();
    dips::extract_thumbnail(input_path, cache_path);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![run_dips, get_thumbnail])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
