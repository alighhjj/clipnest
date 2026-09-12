mod clipboard;
mod commands;
mod models;
mod settings;
mod shortcut;
mod storage;
mod tray;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

use clipboard::ClipboardBridge;
use settings::Settings;
use storage::Storage;

pub const MAIN_WINDOW: &str = "main";
/// 窗口被唤出时通知前端聚焦搜索框
pub const EVENT_FOCUS_SEARCH: &str = "clipnest://focus-search";
/// 设置发生变化
pub const EVENT_SETTINGS_CHANGED: &str = "clipnest://settings-changed";
/// 历史被清空
pub const EVENT_HISTORY_CLEARED: &str = "clipnest://history-cleared";

/// 全局共享状态。
pub struct AppState {
    pub storage: Arc<Storage>,
    pub clipboard: Arc<ClipboardBridge>,
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    // 按下与抬起都会派发，只认按下，否则会连开带关
                    if event.state == ShortcutState::Pressed {
                        toggle_main_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let handle = app.handle().clone();

            let data_dir = handle.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;

            let settings_path = data_dir.join("settings.json");
            let first_run = !settings_path.exists();
            let settings = Settings::load(&settings_path);

            let storage = Arc::new(Storage::new(&data_dir)?);
            let bridge =
                ClipboardBridge::start(handle.clone(), Arc::clone(&storage), &settings);

            // 绑定全局快捷键。失败不致命（可能被别的软件占用），只是没法一键唤出。
            let spec = shortcut::parse(&settings.shortcut)
                .unwrap_or_else(|_| shortcut::default_spec());
            if let Err(err) = shortcut::rebind(&handle, spec) {
                eprintln!("[clipnest] {err}");
            }

            app.manage(AppState {
                storage,
                clipboard: bridge,
                settings: Mutex::new(settings),
                settings_path,
            });

            tray::create(&handle)?;

            // 首次启动亮个相，让用户知道它已经在托盘里跑着了。
            if first_run {
                if let Some(window) = handle.get_webview_window(MAIN_WINDOW) {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            // 无边框窗口：点关闭只是收进托盘，不真的退出
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Focused(false) => {
                let hide = window
                    .app_handle()
                    .try_state::<AppState>()
                    .map(|state| state.settings.lock().hide_on_blur)
                    .unwrap_or(false);
                if hide {
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_clips,
            commands::clip_stats,
            commands::copy_clip,
            commands::activate_clip,
            commands::toggle_pin,
            commands::delete_clip,
            commands::clear_clips,
            commands::get_settings,
            commands::update_settings,
            commands::hide_window,
            commands::app_info,
        ])
        .run(tauri::generate_context!())
        .expect("ClipNest 启动失败");
}

/// 显示 / 隐藏主窗口。托盘左键、全局快捷键、前端按钮都走这里。
pub fn toggle_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };

    let visible = window.is_visible().unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);

    if visible && !minimized {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = app.emit(EVENT_FOCUS_SEARCH, ());
    }
}

/// 统一的应用设置入口：校验 → 按需重绑快捷键 → 同步到监听线程 → 落盘 → 广播。
///
/// 命令层和托盘菜单共用它，保证两边行为一致。
pub fn apply_settings(app: &AppHandle, incoming: Settings) -> Result<Settings, String> {
    let state = app.state::<AppState>();

    let mut next = incoming;
    next.sanitize();

    let shortcut_changed = {
        let current = state.settings.lock();
        current.shortcut != next.shortcut
    };

    if shortcut_changed {
        let spec = shortcut::parse(&next.shortcut)?;
        shortcut::rebind(app, spec)?;
    }

    state.clipboard.set_enabled(next.enabled);
    state
        .clipboard
        .poll_ms
        .store(next.poll_interval_ms, Ordering::Relaxed);
    state
        .clipboard
        .max_items
        .store(next.max_items, Ordering::Relaxed);

    next.save(&state.settings_path)?;
    *state.settings.lock() = next.clone();

    let _ = app.emit(EVENT_SETTINGS_CHANGED, next.clone());
    Ok(next)
}
