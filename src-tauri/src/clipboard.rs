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
                    last_image_error: None,
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
            .send(Request::SetText { text, ack: ack_tx })
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

/// 一轮图片读取的结果。
///
/// 必须区分「剪贴板里本来就没有图片」和「有图片但读失败了」：前者是常态
/// （剪贴板里放的是文本时每轮都会命中），后者才是需要排查的问题。
/// 轮询间隔只有几百毫秒，前者一旦写日志就会刷屏。
enum ImageRead {
    Ready(arboard::ImageData<'static>),
    Empty,
    Failed(String),
}

/// 兜底读取旧式 `CF_DIB`。
///
/// arboard 在 Windows 上只覆盖两种格式：自定义 `PNG` 格式与 `CF_DIBV5`
/// （见 arboard 的 `src/platform/windows.rs`）。但截图工具和不少原生程序
/// 只写 40 字节头的 `CF_DIB`，命中不了就直接返回 `ContentNotAvailable`，
/// 于是「复制了图片却什么都没记录」。
///
/// 返回 `Ok(None)` 表示剪贴板里没有 `CF_DIB`（正常情况）。
#[cfg(windows)]
fn read_legacy_cf_dib() -> Result<Option<arboard::ImageData<'static>>, String> {
    use std::io::Cursor;

    use image::codecs::bmp::BmpDecoder;
    use image::{DynamicImage, ImageDecoder};

    // 查格式不需要打开剪贴板，先挡掉最常见的「本来就没有图片」
    if !clipboard_win::raw::is_format_avail(clipboard_win::formats::CF_DIB) {
        return Ok(None);
    }

    // RAII：构造即 OpenClipboard，离开作用域自动 CloseClipboard
    let opened = clipboard_win::Clipboard::new_attempts(10);
    let _guard = opened.map_err(|e| format!("打开剪贴板失败: {e}"))?;

    let mut raw = Vec::new();
    clipboard_win::raw::get_vec(clipboard_win::formats::CF_DIB, &mut raw)
        .map_err(|e| format!("读取 CF_DIB 失败: {e}"))?;

    if raw.len() < 40 {
        return Err(format!("CF_DIB 数据长度不足：{} 字节", raw.len()));
    }

    // BITMAPINFOHEADER 与 BITMAPV5HEADER 的前 40 字节布局一致：
    // biSize @0，biBitCount @14，biCompression @16。
    let bi_size = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let bit_count = u16::from_le_bytes([raw[14], raw[15]]);
    let compression = u32::from_le_bytes([raw[16], raw[17], raw[18], raw[19]]);

    // 颜色掩码只存在于 V3(56)/V4(108)/V5(124) 头里。40 字节的 BITMAPINFOHEADER
    // 后面直接跟像素数据，那个位置不能按掩码读。
    //
    // bi_size 只是头里声明的值，坏数据可能声明得比实际缓冲还大，所以两个条件都要判，
    // 否则下面按偏移取值会越界 panic。
    if bi_size >= 56 && raw.len() >= 56 {
        tweak_cf_dib_header(&mut raw, bit_count, compression);
    }

    // `new_without_file_header` 正是为 CF_DIB 准备的：同时吃
    // 40 字节 BITMAPINFOHEADER 和 124 字节 BITMAPV5HEADER。
    let decoder = BmpDecoder::new_without_file_header(Cursor::new(raw))
        .map_err(|e| format!("解析 CF_DIB 失败: {e}"))?;
    let (width, height) = decoder.dimensions();
    let mut rgba = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("解码 CF_DIB 失败: {e}"))?
        .into_rgba8();

    // 另一类坏数据：生产者声明了 alpha 掩码，却把 alpha 全填 0。此时整张图
    // 全透明，在历史里就是一张空白缩略图。全透明的位图对剪贴板历史没有意义，
    // 兜底置为不透明，避免存进去一条「看着像坏了」的记录。
    if rgba.pixels().all(|pixel| pixel.0[3] == 0) {
        for pixel in rgba.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
    }

    Ok(Some(arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Owned(rgba.into_raw()),
    }))
}

/// 改写 `CF_DIB` 头部，对齐 arboard 对 `CF_DIBV5` 的处理（`maybe_tweak_header`）。
///
/// MSDN 规定 BI_RGB 下 32bpp 的第 4 个字节「未使用」，但 Chrome / Electron 系
/// 应用会一边写 BI_RGB、一边把真正的 alpha 放进那个字节。`image` 的 BMP 解码器
/// 严格照 MSDN 走，这些图的透明信息会被丢掉。
///
/// 做法是把「明确声明了 alpha 掩码」的情况改写成 BI_BITFIELDS，让解码器走掩码
/// 分支，从而读到真实的 alpha；其余情况不动，保持不透明。
#[cfg(windows)]
fn tweak_cf_dib_header(raw: &mut [u8], bit_count: u16, compression: u32) {
    fn read_u32_at(raw: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([raw[at], raw[at + 1], raw[at + 2], raw[at + 3]])
    }

    const BI_RGB: u32 = 0;
    const BI_BITFIELDS: u32 = 3;
    const ALPHA_MASK: u32 = 0xff00_0000;
    // 补掩码时用的标准 BGRA 排布
    const RED_MASK: u32 = 0x00ff_0000;
    const GREEN_MASK: u32 = 0x0000_ff00;
    const BLUE_MASK: u32 = 0x0000_00ff;
    // BITMAPV5HEADER 里的字段偏移
    const OFF_COMPRESSION: usize = 16;
    const OFF_RED_MASK: usize = 40;
    const OFF_GREEN_MASK: usize = 44;
    const OFF_BLUE_MASK: usize = 48;
    const OFF_ALPHA_MASK: usize = 52;

    if bit_count != 32 || compression != BI_RGB {
        return;
    }
    if read_u32_at(raw, OFF_ALPHA_MASK) != ALPHA_MASK {
        return;
    }

    let rgb_masks_absent = read_u32_at(raw, OFF_RED_MASK) == 0
        && read_u32_at(raw, OFF_GREEN_MASK) == 0
        && read_u32_at(raw, OFF_BLUE_MASK) == 0;

    if rgb_masks_absent {
        raw[OFF_RED_MASK..OFF_RED_MASK + 4].copy_from_slice(&RED_MASK.to_le_bytes());
        raw[OFF_GREEN_MASK..OFF_GREEN_MASK + 4].copy_from_slice(&GREEN_MASK.to_le_bytes());
        raw[OFF_BLUE_MASK..OFF_BLUE_MASK + 4].copy_from_slice(&BLUE_MASK.to_le_bytes());
    }
    raw[OFF_COMPRESSION..OFF_COMPRESSION + 4].copy_from_slice(&BI_BITFIELDS.to_le_bytes());
}

/// 统一的图片读取入口：arboard 优先，Windows 上再兜底旧式 `CF_DIB`。
#[cfg(windows)]
fn read_clipboard_image(board: Option<&mut Clipboard>) -> ImageRead {
    let Some(board) = board else {
        return ImageRead::Failed("剪贴板不可用".into());
    };

    match board.get_image() {
        Ok(image) => ImageRead::Ready(image),
        // 剪贴板里没有 PNG / CF_DIBV5 —— 放的是文本时就是这种结果
        Err(arboard::Error::ContentNotAvailable) => match read_legacy_cf_dib() {
            Ok(Some(image)) => ImageRead::Ready(image),
            Ok(None) => ImageRead::Empty,
            Err(reason) => ImageRead::Failed(reason),
        },
        Err(err) => ImageRead::Failed(format!("arboard 读取失败: {err}")),
    }
}

/// 非 Windows 平台没有 `CF_DIB` 这一层，arboard 说没有就是真的没有。
#[cfg(not(windows))]
fn read_clipboard_image(board: Option<&mut Clipboard>) -> ImageRead {
    let Some(board) = board else {
        return ImageRead::Failed("剪贴板不可用".into());
    };

    match board.get_image() {
        Ok(image) => ImageRead::Ready(image),
        Err(arboard::Error::ContentNotAvailable) => ImageRead::Empty,
        Err(err) => ImageRead::Failed(format!("arboard 读取失败: {err}")),
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
    /// 上一次图片读取失败的原因，用于去重，避免同一错误每轮都刷日志。
    last_image_error: Option<String>,
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
        // 启动瞬间剪贴板里若已有图片，登记为「已知」，避免一开机就把陈年位图抓进历史。
        if let ImageRead::Ready(image) = read_clipboard_image(self.board.as_mut()) {
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
        let image = read_clipboard_image(self.board.as_mut());

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

        match image {
            ImageRead::Ready(image) => {
                // 读到了内容，上一次的故障已恢复，清掉去重记录让下次故障能再报一次
                self.last_image_error = None;

                let digest = hash_hex(&image.bytes);
                if digest == self.last_image_hash {
                    return;
                }

                // 先把指纹记为「已知」，再判断是否处于静默期。
                //
                // 这一步的顺序是刻意的：剪贴板写回后某些平台会重新编码位图，
                // 读回来的字节与写进去的不同，指纹自然也对不上。若把指纹更新
                // 放到静默期判断之后，等静默期一过这条「自己复制自己」的回声
                // 就会被当成新图片入库。代价是静默期内（1.5s）用户真去复制
                // 一张新图会被漏掉一次——两害相权，漏一次远好过每次回写都多
                // 一条重复记录。
                self.last_image_hash = digest;
                if Instant::now() < self.image_guard_until {
                    return;
                }

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
            // 剪贴板里没有图片（放的是文本，或刚被清空）——常态，不记日志
            ImageRead::Empty => {
                self.last_image_error = None;
            }
            // 有图片但读不出来。轮询间隔只有几百毫秒，同一原因只报一次，
            // 否则一行错误会以每秒数条的速度刷屏。
            ImageRead::Failed(reason) => {
                if self.last_image_error.as_deref() != Some(reason.as_str()) {
                    eprintln!("[clipnest] 读取剪贴板图片失败: {reason}");
                    self.last_image_error = Some(reason);
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
