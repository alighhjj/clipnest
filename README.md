# ClipNest

[![CI](https://github.com/YOUR_GITHUB_USER/clipnest/actions/workflows/ci.yml/badge.svg)](https://github.com/YOUR_GITHUB_USER/clipnest/actions/workflows/ci.yml)
[![Release](https://github.com/YOUR_GITHUB_USER/clipnest/actions/workflows/release.yml/badge.svg)](https://github.com/YOUR_GITHUB_USER/clipnest/actions/workflows/release.yml)

> 徽章里的 `YOUR_GITHUB_USER` 换成你的用户名即可。

常驻托盘的跨平台剪贴板历史管理器。用 **Tauri v2 + React + Rust** 写成，
Windows / macOS / Linux 一套代码，安装包体积小、内存占用低。

按一下全局快捷键（默认 `Ctrl+Shift+V`，macOS 为 `⌘⇧V`）弹出一个命令面板式的窗口——
搜索、翻历史、回车即把选中的内容写回剪贴板并自动收起窗口，回到应用里直接粘贴。

**编译交给 CI，本机不需要装 Rust。** 见下面的 [CI/CD](#cicdgithub-actions) 一节。

---

## 功能

| | |
|---|---|
| **历史记录** | 后台轮询监听剪贴板，自动收录文本与图片，按时间去重 |
| **全文搜索** | 边打字边过滤，中文子串匹配（底层用 `instr()`，不依赖分词） |
| **分类筛选** | 全部 / 文本 / 图片 / 置顶，实时显示各类条数 |
| **置顶收藏** | 常用片段钉在最上面，清空历史时可选择保留 |
| **图片支持** | 图片原图 + 缩略图落盘，列表看缩略图、点开看大图 |
| **全局快捷键** | 任意界面一键唤出，可在设置里改成任意组合 |
| **系统托盘** | 左键唤起 / 收起，右键菜单可暂停记录、清空、退出 |
| **纯键盘操作** | `↑` `↓` 选择、`Enter` 复制、`Del` 删除，不用碰鼠标 |
| **自动隐藏** | 窗口失去焦点自动收起（可关），像 Spotlight 一样 |
| **自动淘汰** | 超过保留上限时删除最旧的非置顶条目，同时清掉对应图片文件 |
| **深浅色** | 跟随系统 `prefers-color-scheme` |

## 键盘快捷键（应用内）

| 按键 | 作用 |
|---|---|
| `↑` / `↓` | 上下选择 |
| `Home` / `End` | 跳到首条 / 末条 |
| `Enter` | 写回剪贴板并收起窗口 |
| `Ctrl/Cmd + C` | 只复制，窗口留着 |
| `Ctrl/Cmd + P` | 置顶 / 取消置顶 |
| `Delete` | 删除选中条目 |
| `Esc` | 有搜索词时清空搜索，否则收起窗口 |

## 技术栈

- **前端**：React 18 + TypeScript + Vite，无 UI 框架，纯手写 CSS
- **后端**：Rust + Tauri v2
- **存储**：SQLite（`rusqlite` 的 `bundled` 特性，编译进二进制，用户机器无需装 SQLite）
- **剪贴板**：`arboard`
- **图片**：`image`（PNG 编码 + 缩略图生成）
- **插件**：`tauri-plugin-global-shortcut`

## 目录结构

```
clipnest/
├── src/                        前端
│   ├── App.tsx                 主界面、键盘逻辑、事件订阅
│   ├── api.ts                  invoke 封装
│   ├── types.ts                与 Rust 侧对齐的类型
│   ├── utils.ts                时间 / 体积 / 快捷键格式化
│   ├── styles.css              全部样式（含深浅色变量）
│   └── components/
│       ├── ClipItem.tsx        单条历史卡片
│       ├── SettingsPanel.tsx   设置抽屉
│       ├── Lightbox.tsx        图片大图预览
│       └── Icons.tsx           内联 SVG 图标
├── src-tauri/                  后端
│   ├── src/
│   │   ├── lib.rs              应用装配、窗口逻辑、设置入口
│   │   ├── clipboard.rs        剪贴板轮询线程 + 命令桥
│   │   ├── storage.rs          SQLite 读写、去重、淘汰、图片落盘
│   │   ├── commands.rs         暴露给前端的命令
│   │   ├── shortcut.rs         全局快捷键注册
│   │   ├── settings.rs         配置读写与校验
│   │   ├── tray.rs             托盘图标与菜单
│   │   └── models.rs           数据结构
│   ├── capabilities/default.json
│   ├── tauri.conf.json
│   └── Cargo.toml
└── scripts/
    ├── make-icon.py            生成应用图标源图
    └── verify.py               无 Rust 环境下的静态校验
```

## CI/CD（GitHub Actions）

仓库里已经放好两个工作流，**不需要在本机装 Rust**：

| 文件 | 触发 | 干什么 |
|---|---|---|
| `.github/workflows/ci.yml` | push 到 `main` / `master`、PR、手动 | 静态校验 + 前端类型检查与构建；三个平台各跑一遍 `cargo check`；fmt / clippy 作为提示 |
| `.github/workflows/release.yml` | 推 `v*` 标签，或手动触发 | 四份矩阵构建（macOS 双架构 + Linux + Windows），产出安装包并创建 **草稿** Release |

### 出包流程

```bash
# 本地仓库已初始化并完成首次提交（分支 main），只需接上远端
git remote add origin git@github.com:YOUR_GITHUB_USER/clipnest.git
git push -u origin main

git tag v0.1.0
git push origin v0.1.0      # 这一步就会开始打包
```

几分钟后在仓库的 **Releases** 页面会看到一个草稿，里面挂着：

- `ClipNest_0.1.0_x64-setup.exe` / `.msi`（Windows）
- `ClipNest_0.1.0_aarch64.dmg`、`ClipNest_0.1.0_x64.dmg`（macOS）
- `ClipNest_0.1.0_amd64.AppImage` / `.deb` / `.rpm`（Linux）

确认没问题再点发布。想改口径就让 `release.yml` 里的 `releaseDraft: false`。

### 关于签名

工作流**没有配置代码签名**，所以：

- Windows 首次运行会有 SmartScreen 警告，选「仍要运行」即可。要消掉需配 `TAURI_SIGNING_PRIVATE_KEY` 和证书。
- macOS 直接打开会被 Gatekeeper 拦，右键 →「打开」一次即可。要正式分发需要 Apple Developer 账号和公证（notarization）。

### fmt / clippy 为什么是「提示性」

`cargo fmt --check` 和 `clippy` 那两步挂了 `continue-on-error`，
因为代码是用别的机器写的、没跑过 rustfmt，首次推送很可能有格式差异。

想要硬门禁：在本地（或 CI 的一次性任务里）跑一遍

```bash
cd src-tauri && cargo fmt && cargo clippy --fix --allow-dirty
```

然后把 `ci.yml` 里 `lint` job 的 `continue-on-error` 和两个步骤上的 `continue-on-error` 删掉。

## 环境准备

**只在本地开发时才需要**（纯用 CI 出包可以跳过）：

**Node 18+**，以及 Rust 工具链：

```bash
# 1) 安装 Rust
#    Windows: https://rustup.rs  （需要 C++ 编译器）
#    也可以用 GNU 工具链省掉几个 GB 的 MSVC：
#      scoop install mingw
#      rustup toolchain install stable-x86_64-pc-windows-gnu
#      rustup default stable-x86_64-pc-windows-gnu
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # macOS / Linux

# 2) 系统依赖
#    Windows：WebView2（Win11 自带）、MSVC Build Tools 或 MinGW-w64
#    macOS  ：xcode-select --install
#    Linux  ：libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
#             libxdo-dev build-essential curl wget file libssl-dev
```

## 开发与打包

```bash
npm install
npm run tauri:dev        # 开发模式（热重载）
npm run tauri:build      # 打安装包，产物在 src-tauri/target/release/bundle/
npm run build            # 只做前端类型检查 + 构建
npm run verify           # 静态校验（不需要 Rust 工具链）
```

想换图标：

```bash
python scripts/make-icon.py app-icon.png   # 需要 Pillow
npx tauri icon app-icon.png
```

### 静态校验（`npm run verify`）

没装 Rust 也能查出不少问题，改完配置或加了新命令之后建议跑一遍：

```bash
npm run verify           # 需要 python 及 pyyaml、jsonschema
npm run verify:offline   # 跳过需要联网的 schema 校验
```

`scripts/verify.py` 的检查项：

| 检查项 | 挡掉什么问题 |
|---|---|
| `tauri.conf.json` 对官方 v2 schema | 配置字段写错，否则要等 CI 编译才暴露 |
| `bundle.icon` 列出的文件是否存在 | 图标缺失会让 `generate_context!` 直接编译失败 |
| `frontendDist` 目录是否存在 | 没先 `npm run build` 就 `cargo check` 的经典错误 |
| 前端 `invoke('x')` ↔ 后端 `generate_handler!` | 命令名拼错，只在运行时才报错 |
| capabilities 权限覆盖 | 前端调了某个 window API 却没授权 |
| 工作流里 Rust job 的步骤顺序 | `cargo check` 前漏了建前端，CI 必然失败 |

CI 的 `web` job 里也跑了这一步，把这类问题挡在三个平台的编译矩阵之前（几秒 vs 几十分钟）。

## 设计说明

**为什么只有一个线程碰剪贴板。**
`arboard::Clipboard` 没有给出跨线程的 `Send` 保证，而且多个线程同时读写系统剪贴板容易互相抢所有权。
所以这里把「唯一一个 `Clipboard` 实例」放在专门的轮询线程里，前端命令通过 channel 投递
「把这段文本／这张图写回剪贴板」的请求，线程处理完再回 ack。线程用 `recv_timeout` 等待，
所以复制动作是**立即**执行的，不会被轮询间隔拖延。

**怎么避免「自己复制自己」的回声。**
用户从历史里选中一条时，剪贴板会被写回同样的内容，下一轮轮询就会再抓一次。处理办法有两层：
写回时立刻把该内容的指纹记为「已知」，所以正常情况直接被跳过；图片额外加 1.5 秒静默期，
用来兜住部分平台重新编码位图导致字节不一致的情况。

**去重。**
每条记录存 `sha256` 指纹并加唯一索引。重复内容不新增行，只把它提到最前面——
效果等同于「又复制了一次」，这也是大多数剪贴板管理器的行为。

**启动预热。**
进程启动时先读一次剪贴板并记为已知，避免一开机就把陈年内容抓进历史。

## 数据位置

| 平台 | 路径 |
|---|---|
| Windows | `%APPDATA%\com.clipnest.app\` |
| macOS | `~/Library/Application Support/com.clipnest.app/` |
| Linux | `~/.local/share/com.clipnest.app/` |

- `clips.db` —— SQLite 数据库（正文、指纹、置顶状态）
- `settings.json` —— 配置
- `media/` —— 图片原图；`media/thumbs/` —— 缩略图

删掉整个目录即可完全重置。

## 已知限制 / 待办

- **没有自动粘贴**。选中后是「写回剪贴板 + 收起窗口」，还需要你在目标应用里按一次 `Ctrl+V`。
  自动粘贴要往目标窗口注入按键，各平台做法差别大，暂时不做。
- **只支持文本和图片**，暂不记录「复制的文件列表」。
- **不记录来源应用**（哪个程序复制的）。数据库里预留了位置，但没做前台窗口探测。
- **快捷键输入框接受原始写法**（如 `ctrl+shift+KeyV`）。
  `Ctrl+Shift+V` 这种随手写法会自动补成 `KeyV`，但功能键要写规范名（`Space`、`F1`…）。
- 快捷键被其它软件占用时不会崩溃，只是在设置里改绑时会给出提示。
