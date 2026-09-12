use serde::{Deserialize, Serialize};

/// 剪贴板条目的类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
}

impl ClipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ClipKind::Text => "text",
            ClipKind::Image => "image",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw {
            "image" => ClipKind::Image,
            _ => ClipKind::Text,
        }
    }
}

/// 暴露给前端的条目结构。字段名统一转成 camelCase。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: i64,
    pub kind: ClipKind,
    /// 文本条目为原文；图片条目为磁盘文件名（不含扩展名）
    pub content: String,
    /// 列表里展示的单行/多行摘要
    pub preview: String,
    pub byte_size: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub pinned: bool,
    pub created_at: i64,
    /// 图片条目的原图绝对路径，供前端 convertFileSrc 使用
    pub image_path: Option<String>,
    /// 图片条目的缩略图绝对路径
    pub thumb_path: Option<String>,
}

/// 列表查询条件。
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipQuery {
    pub search: Option<String>,
    /// all | text | image | pinned
    pub filter: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipStats {
    pub total: i64,
    pub text: i64,
    pub image: i64,
    pub pinned: i64,
}
