use std::str::FromStr;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

/// 各平台的默认唤出快捷键：macOS 用 ⌘⇧V，其余平台用 Ctrl+Shift+V。
pub fn default_spec() -> Shortcut {
    #[cfg(target_os = "macos")]
    let mods = Modifiers::SUPER | Modifiers::SHIFT;
    #[cfg(not(target_os = "macos"))]
    let mods = Modifiers::CONTROL | Modifiers::SHIFT;

    Shortcut::new(Some(mods), Code::KeyV)
}

/// 把 `Ctrl+Shift+V` 这类随手写法补成插件认识的 `ctrl+shift+KeyV`。
fn normalize(input: &str) -> String {
    input
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| {
            let lower = token.to_ascii_lowercase();
            match lower.as_str() {
                "ctrl" | "control" => "control".to_string(),
                "shift" => "shift".to_string(),
                "alt" | "option" => "alt".to_string(),
                "meta" | "super" | "cmd" | "command" | "win" => "super".to_string(),
                _ => {
                    if token.len() == 1 && token.chars().all(|c| c.is_ascii_alphabetic()) {
                        format!("Key{}", token.to_ascii_uppercase())
                    } else if token.len() == 1 && token.chars().all(|c| c.is_ascii_digit()) {
                        format!("Digit{token}")
                    } else {
                        token.to_string()
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

pub fn parse(input: &str) -> Result<Shortcut, String> {
    Shortcut::from_str(&normalize(input)).map_err(|_| {
        format!("无法识别快捷键「{input}」。参考写法：ctrl+shift+KeyV 或 ctrl+alt+Space")
    })
}

/// 换绑全局快捷键。会先解绑旧的，避免同一程序里的重复注册报错。
pub fn rebind(app: &AppHandle, spec: Shortcut) -> Result<(), String> {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    manager
        .register(spec)
        .map_err(|e| format!("注册快捷键失败，可能已被其他程序占用: {e}"))
}
