use std::fs;
use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::models::{Clip, ClipKind, ClipQuery, ClipStats};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS clips (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    kind       TEXT    NOT NULL,
    content    TEXT    NOT NULL,
    preview    TEXT    NOT NULL,
    hash       TEXT    NOT NULL UNIQUE,
    byte_size  INTEGER NOT NULL DEFAULT 0,
    width      INTEGER,
    height     INTEGER,
    pinned     INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_clips_created ON clips(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_clips_pinned  ON clips(pinned DESC, created_at DESC);
"#;

/// 单条文本最多保存的字节数，超过部分截断，避免数据库被超大文本撑爆。
const MAX_TEXT_BYTES: usize = 512 * 1024;

/// 缩略图最长边。
const THUMB_SIZE: u32 = 360;

pub struct Storage {
    conn: Mutex<Connection>,
    media_dir: PathBuf,
    thumb_dir: PathBuf,
}

impl Storage {
    /// 在 `<data_dir>/clips.db` 建立数据库，图片落在 `<data_dir>/media`。
    pub fn new(data_dir: &Path) -> Result<Self, String> {
        let media_dir = data_dir.join("media");
        let thumb_dir = media_dir.join("thumbs");
        fs::create_dir_all(&thumb_dir).map_err(|e| format!("创建媒体目录失败: {e}"))?;

        let db_path = data_dir.join("clips.db");
        let conn = Connection::open(&db_path).map_err(|e| format!("打开数据库失败: {e}"))?;

        // WAL 能显著改善读写并发；某些文件系统不支持时静默降级。
        let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");
        let _ = conn.execute_batch("PRAGMA synchronous=NORMAL;");
        conn.execute_batch(SCHEMA)
            .map_err(|e| format!("初始化表结构失败: {e}"))?;

        Ok(Self {
            conn: Mutex::new(conn),
            media_dir,
            thumb_dir,
        })
    }

    pub fn media_dir(&self) -> &Path {
        &self.media_dir
    }

    // ---------------------------------------------------------------- 查询

    pub fn list(&self, query: &ClipQuery) -> Result<Vec<Clip>, String> {
        let filter = query.filter.clone().unwrap_or_else(|| "all".into());
        let search = query
            .search
            .clone()
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let limit = query.limit.unwrap_or(300).clamp(1, 2000);
        let offset = query.offset.unwrap_or(0).max(0);

        let sql = r#"
            SELECT id, kind, content, preview, byte_size, width, height, pinned, created_at
            FROM clips
            WHERE (
                    ?1 = 'all'
                 OR (?1 = 'text'   AND kind = 'text')
                 OR (?1 = 'image'  AND kind = 'image')
                 OR (?1 = 'pinned' AND pinned = 1)
            )
              AND (?2 = '' OR instr(lower(content), ?2) > 0 OR instr(lower(preview), ?2) > 0)
            ORDER BY pinned DESC, created_at DESC
            LIMIT ?3 OFFSET ?4
        "#;

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![filter, search, limit, offset], |row| {
                Ok(RawRow {
                    id: row.get(0)?,
                    kind: row.get::<_, String>(1)?,
                    content: row.get(2)?,
                    preview: row.get(3)?,
                    byte_size: row.get(4)?,
                    width: row.get(5)?,
                    height: row.get(6)?,
                    pinned: row.get::<_, i64>(7)? != 0,
                    created_at: row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for raw in rows {
            let raw = raw.map_err(|e| e.to_string())?;
            out.push(self.hydrate(raw));
        }
        Ok(out)
    }

    pub fn get(&self, id: i64) -> Result<Option<Clip>, String> {
        let conn = self.conn.lock();
        let raw = conn
            .query_row(
                "SELECT id, kind, content, preview, byte_size, width, height, pinned, created_at
                 FROM clips WHERE id = ?1",
                params![id],
                |row| {
                    Ok(RawRow {
                        id: row.get(0)?,
                        kind: row.get::<_, String>(1)?,
                        content: row.get(2)?,
                        preview: row.get(3)?,
                        byte_size: row.get(4)?,
                        width: row.get(5)?,
                        height: row.get(6)?,
                        pinned: row.get::<_, i64>(7)? != 0,
                        created_at: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;

        Ok(raw.map(|r| self.hydrate(r)))
    }

    pub fn stats(&self) -> Result<ClipStats, String> {
        let conn = self.conn.lock();
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM clips", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let text: i64 = conn
            .query_row("SELECT COUNT(*) FROM clips WHERE kind = 'text'", [], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        let image: i64 = conn
            .query_row("SELECT COUNT(*) FROM clips WHERE kind = 'image'", [], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        let pinned: i64 = conn
            .query_row("SELECT COUNT(*) FROM clips WHERE pinned = 1", [], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;

        Ok(ClipStats {
            total,
            text,
            image,
            pinned,
        })
    }

    // ---------------------------------------------------------------- 写入

    /// 写入一条文本。返回 `None` 表示内容为空或与「最近一条完全一致」，无需记录。
    pub fn push_text(&self, text: &str, max_items: i64) -> Result<Option<Clip>, String> {
        if text.trim().is_empty() {
            return Ok(None);
        }

        let mut stored = text.to_string();
        if stored.len() > MAX_TEXT_BYTES {
            // 按字符边界安全截断
            let mut end = MAX_TEXT_BYTES;
            while end > 0 && !stored.is_char_boundary(end) {
                end -= 1;
            }
            stored.truncate(end);
        }

        let hash = hash_hex(stored.as_bytes());
        let preview = make_preview(&stored);
        let byte_size = stored.len() as i64;
        let now = now_ms();

        if let Some(clip) = self.bump_existing(&hash, now)? {
            return Ok(Some(clip));
        }

        let id = {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT INTO clips (kind, content, preview, hash, byte_size, pinned, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6)",
                params![
                    ClipKind::Text.as_str(),
                    stored,
                    preview,
                    hash,
                    byte_size,
                    now
                ],
            )
            .map_err(|e| e.to_string())?;
            conn.last_insert_rowid()
        };

        self.enforce_limit(max_items)?;
        self.get(id)
    }

    /// 写入一条图片。`rgba` 为 4 通道原始像素。
    pub fn push_image(
        &self,
        rgba: &[u8],
        width: u32,
        height: u32,
        max_items: i64,
    ) -> Result<Option<Clip>, String> {
        if width == 0 || height == 0 || rgba.is_empty() {
            return Ok(None);
        }

        let expected = (width as usize) * (height as usize) * 4;
        if rgba.len() < expected {
            return Err("图片像素数据长度不足".into());
        }

        let hash = hash_hex(rgba);
        let now = now_ms();

        if let Some(clip) = self.bump_existing(&hash, now)? {
            return Ok(Some(clip));
        }

        let stem = hash.clone();
        let full_path = self.media_dir.join(format!("{stem}.png"));
        let thumb_path = self.thumb_dir.join(format!("{stem}.png"));

        // 落盘：原图 + 缩略图
        {
            let img = image::RgbaImage::from_raw(width, height, rgba[..expected].to_vec())
                .ok_or_else(|| "无法构造图片缓冲区".to_string())?;
            img.save(&full_path)
                .map_err(|e| format!("保存原图失败: {e}"))?;
            let thumb = image::imageops::thumbnail(&img, THUMB_SIZE, THUMB_SIZE);
            thumb
                .save(&thumb_path)
                .map_err(|e| format!("保存缩略图失败: {e}"))?;
        }

        let preview = format!("图片 {width} × {height}");
        let byte_size = expected as i64;

        let id = {
            let conn = self.conn.lock();
            conn.execute(
                "INSERT INTO clips (kind, content, preview, hash, byte_size, width, height, pinned, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?8)",
                params![
                    ClipKind::Image.as_str(),
                    stem,
                    preview,
                    hash,
                    byte_size,
                    width as i64,
                    height as i64,
                    now
                ],
            )
            .map_err(|e| e.to_string())?;
            conn.last_insert_rowid()
        };

        self.enforce_limit(max_items)?;
        self.get(id)
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE clips SET pinned = ?1, updated_at = ?2 WHERE id = ?3",
            params![if pinned { 1 } else { 0 }, now_ms(), id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<(), String> {
        let mut victims = Vec::new();
        {
            let conn = self.conn.lock();
            if let Ok(Some(stem)) = conn
                .query_row(
                    "SELECT content FROM clips WHERE id = ?1 AND kind = 'image'",
                    params![id],
                    |r| r.get::<_, String>(0),
                )
                .optional()
            {
                victims.push(stem);
            }
            conn.execute("DELETE FROM clips WHERE id = ?1", params![id])
                .map_err(|e| e.to_string())?;
        }
        self.purge_media(&victims);
        Ok(())
    }

    /// 清空历史，返回删除条数。
    pub fn clear(&self, keep_pinned: bool) -> Result<usize, String> {
        let victims: Vec<String> = {
            let conn = self.conn.lock();
            let sql = if keep_pinned {
                "SELECT content FROM clips WHERE kind = 'image' AND pinned = 0"
            } else {
                "SELECT content FROM clips WHERE kind = 'image'"
            };
            let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        };

        let removed = {
            let conn = self.conn.lock();
            let sql = if keep_pinned {
                "DELETE FROM clips WHERE pinned = 0"
            } else {
                "DELETE FROM clips"
            };
            conn.execute(sql, []).map_err(|e| e.to_string())?
        };

        self.purge_media(&victims);
        Ok(removed)
    }

    // ---------------------------------------------------------------- 内部

    /// 命中已有 hash 时，把它提到最前面（等价于「重新复制了一次」）。
    fn bump_existing(&self, hash: &str, now: i64) -> Result<Option<Clip>, String> {
        let conn = self.conn.lock();
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM clips WHERE hash = ?1",
                params![hash],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        let Some(id) = existing else {
            return Ok(None);
        };

        conn.execute(
            "UPDATE clips SET created_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(|e| e.to_string())?;
        drop(conn);

        self.get(id)
    }

    /// 裁剪历史：超过上限时删掉最旧的非置顶条目。
    fn enforce_limit(&self, max_items: i64) -> Result<(), String> {
        let max_items = max_items.clamp(20, 20_000);
        let victims: Vec<String> = {
            let conn = self.conn.lock();
            let mut stmt = conn
                .prepare(
                    "SELECT content FROM clips
                     WHERE kind = 'image' AND pinned = 0 AND id IN (
                         SELECT id FROM clips WHERE pinned = 0
                         ORDER BY created_at DESC LIMIT -1 OFFSET ?1
                     )",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![max_items], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        };

        {
            let conn = self.conn.lock();
            conn.execute(
                "DELETE FROM clips WHERE pinned = 0 AND id IN (
                     SELECT id FROM clips WHERE pinned = 0
                     ORDER BY created_at DESC LIMIT -1 OFFSET ?1
                 )",
                params![max_items],
            )
            .map_err(|e| e.to_string())?;
        }

        self.purge_media(&victims);
        Ok(())
    }

    fn purge_media(&self, stems: &[String]) {
        for stem in stems {
            let _ = fs::remove_file(self.media_dir.join(format!("{stem}.png")));
            let _ = fs::remove_file(self.thumb_dir.join(format!("{stem}.png")));
        }
    }

    fn hydrate(&self, raw: RawRow) -> Clip {
        let kind = ClipKind::parse(&raw.kind);
        let (image_path, thumb_path) = match kind {
            ClipKind::Image => (
                Some(
                    self.media_dir
                        .join(format!("{}.png", raw.content))
                        .to_string_lossy()
                        .into_owned(),
                ),
                Some(
                    self.thumb_dir
                        .join(format!("{}.png", raw.content))
                        .to_string_lossy()
                        .into_owned(),
                ),
            ),
            ClipKind::Text => (None, None),
        };

        Clip {
            id: raw.id,
            kind,
            content: raw.content,
            preview: raw.preview,
            byte_size: raw.byte_size,
            width: raw.width,
            height: raw.height,
            pinned: raw.pinned,
            created_at: raw.created_at,
            image_path,
            thumb_path,
        }
    }
}

struct RawRow {
    id: i64,
    kind: String,
    content: String,
    preview: String,
    byte_size: i64,
    width: Option<i64>,
    height: Option<i64>,
    pinned: bool,
    created_at: i64,
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn hash_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// 生成列表摘要：折叠空白，最长 240 个字符。
fn make_preview(text: &str) -> String {
    let collapsed: String = text
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if collapsed.chars().count() <= 240 {
        return collapsed;
    }
    let head: String = collapsed.chars().take(240).collect();
    format!("{head}…")
}
