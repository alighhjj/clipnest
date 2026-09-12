use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::models::{Clip, ClipKind, ClipQuery, ClipStats};
use crate::settings::Settings;
use crate::{apply_settings, AppState, EVENT_HISTORY_CLEARED};

/// 列出历史条目。`query` 为空时按默认条件返回最近 300 条。
#[tauri::command]
pub fn list_clips(state: State<'_, AppState>, query: Option<ClipQuery>) -> Result<Vec<Clip>, String> {
    state.storage.list(&query.unwrap_or_default())
}

#[tauri::command]
pub fn clip_stats(state: State<'_, AppState>) -> Result<ClipStats, String> {
    state.storage.stats()
}

/// 把某条历史写回系统剪贴板（窗口保持显示）。
#[tauri::command]
pub fn copy_clip(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    write_clip(&state, id)
}

/// 选中某条历史：写回剪贴板并收起窗口，让用户直接去目标应用粘贴。
#[tauri::command]
pub fn activate_clip(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), String> {
    write_clip(&state, id)?;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn toggle_pin(state: State<'_, AppState>, id: i64, pinned: bool) -> Result<(), String> {
    state.storage.set_pinned(id, pinned)
}

#[tauri::command]
pub fn delete_clip(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.storage.delete(id)
}

#[tauri::command]
pub fn clear_clips(
    app: AppHandle,
    state: State<'_, AppState>,
    keep_pinned: bool,
) -> Result<i64, String> {
    let removed = state.storage.clear(keep_pinned)?;
    state.clipboard.reset_fingerprints();
    let _ = app.emit(EVENT_HISTORY_CLEARED, removed as i64);
    Ok(removed as i64)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().clone()
}

#[tauri::command]
pub fn update_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    apply_settings(&app, settings)
}

#[tauri::command]
pub fn hide_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

/// 设置面板里展示的运行信息。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub media_dir: String,
    pub clipboard_ready: bool,
}

#[tauri::command]
pub fn app_info(app: AppHandle, state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: state
            .settings_path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
        media_dir: state.storage.media_dir().to_string_lossy().into_owned(),
        clipboard_ready: state.clipboard.enabled.load(Ordering::Relaxed),
    }
}

fn write_clip(state: &State<'_, AppState>, id: i64) -> Result<(), String> {
    let clip = state
        .storage
        .get(id)?
        .ok_or_else(|| "这条记录已经不存在了".to_string())?;

    match clip.kind {
        // clip 是本地所有权，可以直接把字段移出去，省一次大文本拷贝
        ClipKind::Text => state.clipboard.write_text(clip.content),
        ClipKind::Image => {
            let path = clip
                .image_path
                .ok_or_else(|| "图片文件路径缺失".to_string())?;
            let decoded = image::open(&path).map_err(|e| format!("读取图片失败: {e}"))?;
            let rgba = decoded.to_rgba8();
            let (width, height) = rgba.dimensions();
            state
                .clipboard
                .write_image(width as usize, height as usize, rgba.into_raw())
        }
    }
}
