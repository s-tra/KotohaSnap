pub mod commands;
pub mod config;
pub mod history;
pub mod image_utils;
pub mod osc;
pub mod state;
pub mod translator;
pub mod watcher;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cfg = config::load_config().unwrap_or_else(|e| {
        eprintln!("設定の読み込みに失敗しました（デフォルト使用）: {e}");
        config::Config::default()
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .manage(AppState::new(cfg))
        .setup(|app| {
            watcher::spawn_watcher(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::set_enabled,
            commands::set_osc_enabled,
            commands::open_settings,
            commands::fetch_models,
            commands::get_history,
            commands::clear_history,
            commands::test_osc,
            commands::open_file,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri アプリの起動に失敗しました");
}
