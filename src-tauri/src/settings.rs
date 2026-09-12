use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::shortcut;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// 是否记录新的剪贴板内容
    pub enabled: bool,
    /// 轮询间隔（毫秒）
    pub poll_interval_ms: u64,
    /// 非置顶条目的保留上限
    pub max_items: i64,
    /// 全局快捷键，形如 `ctrl+shift+KeyV`
    pub shortcut: String,
    /// 窗口失去焦点时自动隐藏
    pub hide_on_blur: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_ms: 700,
            max_items: 500,
            shortcut: shortcut::default_spec().to_string(),
            hide_on_blur: true,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(raw) => {
                let mut parsed: Self = serde_json::from_str(&raw).unwrap_or_default();
                parsed.sanitize();
                parsed
            }
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
        }
        let payload =
            serde_json::to_string_pretty(self).map_err(|e| format!("序列化配置失败: {e}"))?;
        fs::write(path, payload).map_err(|e| format!("写入配置失败: {e}"))
    }

    /// 把越界的值拉回合理区间，并保证快捷键非空。
    pub fn sanitize(&mut self) {
        self.poll_interval_ms = self.poll_interval_ms.clamp(200, 10_000);
        self.max_items = self.max_items.clamp(20, 20_000);
        self.shortcut = self.shortcut.trim().to_string();
        if self.shortcut.is_empty() {
            self.shortcut = shortcut::default_spec().to_string();
        }
    }
}
