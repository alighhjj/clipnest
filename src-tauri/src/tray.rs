use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::AppState;

const MENU_TOGGLE: &str = "clipnest.toggle";
const MENU_PAUSE: &str = "clipnest.pause";
const MENU_CLEAR: &str = "clipnest.clear";
const MENU_QUIT: &str = "clipnest.quit";

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, MENU_TOGGLE, "显示 / 隐藏窗口", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, MENU_PAUSE, "暂停 / 继续记录", true, None::<&str>)?;
    let sep_top = PredefinedMenuItem::separator(app)?;
    let clear = MenuItem::with_id(app, MENU_CLEAR, "清空历史（保留置顶）", true, None::<&str>)?;
    let sep_bottom = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出 ClipNest", true, None::<&str>)?;

    // 逐个 append 而不是 with_items(&[..])：菜单项类型不同，数组会类型不一致。
    let menu = Menu::new(app)?;
    menu.append(&toggle)?;
    menu.append(&pause)?;
    menu.append(&sep_top)?;
    menu.append(&clear)?;
    menu.append(&sep_bottom)?;
    menu.append(&quit)?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("ClipNest · 剪贴板历史")
        .on_menu_event(|app, event| handle_menu(app, event.id().0.as_str()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::toggle_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}

fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        MENU_TOGGLE => crate::toggle_main_window(app),

        MENU_PAUSE => {
            let state = app.state::<AppState>();
            let mut next = state.settings.lock().clone();
            next.enabled = !next.enabled;
            if let Err(err) = crate::apply_settings(app, next) {
                eprintln!("[clipnest] 切换记录状态失败: {err}");
            }
        }

        MENU_CLEAR => {
            let state = app.state::<AppState>();
            match state.storage.clear(true) {
                Ok(removed) => {
                    state.clipboard.reset_fingerprints();
                    let _ = app.emit(crate::EVENT_HISTORY_CLEARED, removed as i64);
                }
                Err(err) => eprintln!("[clipnest] 清空历史失败: {err}"),
            }
        }

        MENU_QUIT => app.exit(0),

        _ => {}
    }
}
