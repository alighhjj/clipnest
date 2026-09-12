use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use tauri::{AppHandle, Emitter};

use crate::models::Clip;
use crate::settings::Settings;
use crate::storage::{hash_hex, Storage};

/// 图片写回后的一小段静默期。
///
/// 某些平台会重新编码位图，导致读回来的字节和我们写进去的不完全一致，
/// 从而在历史里多出一条「自己复制自己」的回声条目。静默期内只更新指纹、不记录。
const IMAGE_ECHO_GUARD: Duration = Duration::from_millis(1500);

pub const EVENT_CLIP_ADDED: &str = "clipnest://clip-added";

enum Request {
    SetText {
        text: String,
        ack: Sender<Result<(), String>>,
    },
    SetImage {
        width: usize,
        height: usize,
        rgba: Vec<u8>,
        ack: Sender<Result<(), String>>,
    },
    /// 历史被清空后重置指纹，让当前剪贴板内容可以被重新记录。
    Reset,
}

/// 命令层与轮询线程之间的桥。
///
/// 真正触碰系统剪贴板的永远只有轮询线程那一个 `Clipboard` 实例 —— 这样既避开了
/// 跨平台 `Clipboard` 的 Send/Sync 不确定性，也躲开了多线程抢占剪贴板所有权的坑。
pub struct ClipboardBridge {
    tx: Sender<Request>,
    pub enabled: AtomicBool,
    pub poll_ms: AtomicU64,
    pub max_items: AtomicI64,
}

impl ClipboardBridge {
    pub fn start(app: AppHandle, storage: Arc<Storage>, settings: &Settings) -> Arc<Self> {
        let (tx, rx) = mpsc::channel::<Request>();

        let bridge = Arc::new(Self {
            tx,
            enabled: AtomicBool::new(settings.enabled),
            poll_ms: AtomicU64::new(settings.poll_interval_ms),
            max_items: AtomicI64::new(settings.max_items),
        });

        let thread_bridge = Arc::clone(&bridge);

        // 注意：`arboard::Clipboard` 实例必须在工作线程内部创建，
        // 不能跨线程移动（上游没有保证 Send）。
        let spawned = thread::Builder::new()
            .name("clipnest-clipboard".into())
            .spawn(move || {
                let worker = Worker {
                    app,
                    storage,
                    bridge: thread_bridge,
                    board: Clipboard::new().ok(),
                    last_text_hash: String::new(),
                    last_image_hash: String::new(),
                    image_guard_until: Instant::now(),
                    next_poll: Instant::now(),
                };
                worker.run(rx);
            });

        if let Err(err) = spawned {
            eprintln!("[clipnest] 无法启动剪贴板监听线程: {err}");
        }

        bridge
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, value: bool) {
        self.enabled.store(value, Ordering::Relaxed);
    }

    pub fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.poll_ms.load(Ordering::Relaxed).clamp(200, 10_000))
    }

    pub fn max_items(&self) -> i64 {
        self.max_items.load(Ordering::Relaxed)
    }

    /// 把文本写回系统剪贴板，等待轮询线程确认。
    pub fn write_text(&self, text: String) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(Request::SetText {
                text,
                ack: ack_tx,
            })
            .map_err(|_| "剪贴板监听线程已退出".to_string())?;
        ack_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "写入剪贴板超时".to_string())?
    }

    /// 把图片写回系统剪贴板。
    pub fn write_image(&self, width: usize, height: usize, rgba: Vec<u8>) -> Result<(), String> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(Request::SetImage {
                width,
                height,
                rgba,
                ack: ack_tx,
            })
            .map_err(|_| "剪贴板监听线程已退出".to_string())?;
        ack_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "写入剪贴板超时".to_string())?
    }

    pub fn reset_fingerprints(&self) {
        let _ = self.tx.send(Request::Reset);
    }
}

struct Worker {
    app: AppHandle,
    storage: Arc<Storage>,
    bridge: Arc<ClipboardBridge>,
    board: Option<Clipboard>,
    last_text_hash: String,
    last_image_hash: String,
    image_guard_until: Instant,
    next_poll: Instant,
}

impl Worker {
    fn run(mut self, rx: Receiver<Request>) {
        if self.board.is_none() {
            eprintln!("[clipnest] 剪贴板初始化失败，暂时只能查看历史，无法写入");
        }

        // 预热：把启动瞬间剪贴板里的旧内容记为「已知」，
        // 免得一开机就把陈年内容抓进历史。
        self.prime();
        self.next_poll = Instant::now() + self.bridge.poll_interval();

        loop {
            let wait = self.next_poll.saturating_duration_since(Instant::now());
            match rx.recv_timeout(wait.max(Duration::from_millis(1))) {
                Ok(request) => self.handle(request),
                Err(RecvTimeoutError::Timeout) => {
                    if Instant::now() < self.next_poll {
                        continue;
                    }
                    self.next_poll = Instant::now() + self.bridge.poll_interval();
                    self.tick();
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn prime(&mut self) {
        if let Some(text) = self.board.as_mut().and_then(|b| b.get_text().ok()) {
            if !text.trim().is_empty() {
                self.last_text_hash = hash_hex(text.as_bytes());
            }
        }
        if let Some(image) = self.board.as_mut().and_then(|b| b.get_image().ok()) {
            self.last_image_hash = hash_hex(&image.bytes);
        }
    }

    fn tick(&mut self) {
        if self.board.is_none() {
            // 有机会就重试一次（例如 Linux 上 X11 稍后才起来）
            self.board = Clipboard::new().ok();
            if self.board.is_none() {
                return;
            }
        }
        if !self.bridge.is_enabled() {
            return;
        }

        // 文本与图片各自读一次；两个调用都是即取即走，不长期借用 self.board。
        let text = self.board.as_mut().and_then(|b| b.get_text().ok());
        let image = self.board.as_mut().and_then(|b| b.get_image().ok());

        if let Some(text) = text {
            if !text.trim().is_empty() {
                let digest = hash_hex(text.as_bytes());
                if digest != self.last_text_hash {
                    self.last_text_hash = digest;
                    self.last_image_hash.clear();
                    match self.storage.push_text(&text, self.bridge.max_items()) {
                        Ok(Some(clip)) => self.emit(clip),
                        Ok(None) => {}
                        Err(err) => eprintln!("[clipnest] 保存文本失败: {err}"),
                    }
                    return;
                }
            }
        }

        if let Some(image) = image {
            let digest = hash_hex(&image.bytes);
            if digest != self.last_image_hash {
                self.last_image_hash = digest;
                if Instant::now() >= self.image_guard_until {
                    match self.storage.push_image(
                        &image.bytes,
                        image.width as u32,
                        image.height as u32,
                        self.bridge.max_items(),
                    ) {
                        Ok(Some(clip)) => self.emit(clip),
                        Ok(None) => {}
                        Err(err) => eprintln!("[clipnest] 保存图片失败: {err}"),
                    }
                }
            }
        }
    }

    fn handle(&mut self, request: Request) {
        match request {
            Request::SetText { text, ack } => {
                let result = self.set_text(text);
                let _ = ack.send(result);
            }
            Request::SetImage {
                width,
                height,
                rgba,
                ack,
            } => {
                let result = self.set_image(width, height, rgba);
                let _ = ack.send(result);
            }
            Request::Reset => {
                self.last_text_hash.clear();
                self.last_image_hash.clear();
                self.image_guard_until = Instant::now();
            }
        }
    }

    fn set_text(&mut self, text: String) -> Result<(), String> {
        let digest = hash_hex(text.as_bytes());
        let Some(board) = self.board.as_mut() else {
            return Err("剪贴板不可用，无法写入".into());
        };
        board
            .set_text(text)
            .map_err(|e| format!("写入剪贴板失败: {e}"))?;
        self.last_text_hash = digest;
        Ok(())
    }

    fn set_image(&mut self, width: usize, height: usize, rgba: Vec<u8>) -> Result<(), String> {
        let digest = hash_hex(&rgba);
        let Some(board) = self.board.as_mut() else {
            return Err("剪贴板不可用，无法写入".into());
        };
        board
            .set_image(arboard::ImageData {
                width,
                height,
                bytes: Cow::Owned(rgba),
            })
            .map_err(|e| format!("写入剪贴板失败: {e}"))?;
        self.last_image_hash = digest;
        self.image_guard_until = Instant::now() + IMAGE_ECHO_GUARD;
        Ok(())
    }

    fn emit(&self, clip: Clip) {
        let _ = self.app.emit(EVENT_CLIP_ADDED, clip);
    }
}
