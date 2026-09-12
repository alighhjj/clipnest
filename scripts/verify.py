#!/usr/bin/env python3
"""ClipNest 无 Rust 环境下的静态校验。

在没有 Rust 工具链的机器上，尽可能把「不需要编译器就能发现」的问题找出来：
配置文件 schema、图标资源完整性、前后端命令名对齐、权限是否覆盖前端 API 调用。

用法：
    python scripts/verify.py            # 校验，失败时退出码 1
    python scripts/verify.py --no-net   # 跳过需要联网的 schema 校验（离线）
"""

from __future__ import annotations

import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TAURI = ROOT / "src-tauri"
CACHE = ROOT / ".cache-schema"

TAURI_CONFIG_SCHEMA = "https://schema.tauri.app/config/2"

# 前端用到这些 API 时，capabilities 里必须有的权限
API_TO_PERMISSION = {
    "startDragging": "core:window:allow-start-dragging",
    "minimize": "core:window:allow-minimize",
    "hide": "core:window:allow-hide",
    "show": "core:window:allow-show",
    "close": "core:window:allow-close",
    "setFocus": "core:window:allow-set-focus",
}

# Rust 侧是否在用某个窗口 API。注意：Rust 侧调用不经过 ACL/IPC 权限检查，
# 所以这里只用来区分「这条授权是冗余」还是「是给 Rust 预留的冗余」。
RUST_API_HINTS = {
    "core:window:allow-hide": r"\.hide\s*\(\s*\)",
    "core:window:allow-show": r"\.show\s*\(\s*\)",
    "core:window:allow-set-focus": r"\.set_focus\s*\(\s*\)",
    "core:window:allow-close": r"\.close\s*\(\s*\)",
    "core:window:allow-minimize": r"\.minimize\s*\(\s*\)",
    "core:window:allow-start-dragging": r"start_dragging\s*\(\s*\)",
}

ok: list[str] = []
warn: list[str] = []
fail: list[str] = []


def report(level: str, msg: str) -> None:
    {"ok": ok, "warn": warn, "fail": fail}[level].append(msg)
    print({"ok": "  OK   ", "warn": "  WARN ", "fail": "  FAIL "}[level] + msg)


def section(title: str) -> None:
    print(f"\n=== {title} ===")


# ---------------------------------------------------------------- 纯文本/JSON

def load_json(path: Path) -> object | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        report("fail", f"文件缺失：{path.relative_to(ROOT)}")
    except json.JSONDecodeError as exc:
        report("fail", f"JSON 解析失败 {path.relative_to(ROOT)}: {exc}")
    return None


def check_config() -> dict | None:
    section("tauri.conf.json")
    cfg = load_json(TAURI / "tauri.conf.json")
    if not isinstance(cfg, dict):
        return None

    identifier = cfg.get("identifier", "")
    if identifier in ("", "com.tauri.dev"):
        report("fail", f"identifier 不能是默认值（当前 {identifier!r}），打包会直接失败")
    else:
        report("ok", f"identifier = {identifier}")

    dist = (TAURI / cfg.get("build", {}).get("frontendDist", "")).resolve()
    if dist.is_dir():
        report("ok", f"frontendDist 存在：{dist.relative_to(ROOT)}")
    else:
        # 这不是错误信息本身，但 tauri-codegen 会因此编译失败
        report("fail", f"frontendDist 目录不存在：{dist}（先跑 npm run build）")

    icons = cfg.get("bundle", {}).get("icon", [])
    if not icons:
        report("fail", "bundle.icon 为空，generate_context! 会失败")
    for icon in icons:
        if (TAURI / icon).is_file():
            report("ok", f"图标 {icon}")
        else:
            report("fail", f"bundle.icon 列出的文件不存在：{icon}")

    windows = cfg.get("app", {}).get("windows", [])
    if windows:
        report("ok", f"app.windows 定义 {len(windows)} 个窗口")
    else:
        report("warn", "app.windows 为空，将由 Rust 侧创建窗口")

    # 托盘常驻应用：窗口不应自带任务栏图标，也不该在关闭时退出
    for i, win in enumerate(windows):
        label = win.get("label", "main")
        if win.get("skipTaskbar") is False and win.get("visible", True) is False:
            report("warn", f"窗口 {i} ({label}) 隐藏但未跳过任务栏")
    return cfg


def check_workflows() -> None:
    section("GitHub Actions 工作流")
    try:
        import yaml
    except ImportError:
        report("warn", "未安装 PyYAML，跳过工作流语法校验")
        return

    wf_dir = ROOT / ".github" / "workflows"
    if not wf_dir.is_dir():
        report("fail", "缺少 .github/workflows/")
        return

    for path in sorted(wf_dir.glob("*.y*ml")):
        try:
            doc = yaml.safe_load(path.read_text(encoding="utf-8"))
        except yaml.YAMLError as exc:
            report("fail", f"{path.name} YAML 语法错误：{exc}")
            continue
        if not isinstance(doc, dict):
            report("fail", f"{path.name} 顶层不是映射")
            continue
        jobs = doc.get("jobs", {})
        report("ok", f"{path.name} 解析通过，{len(jobs)} 个 job：{', '.join(jobs)}")

        for job_name, job in jobs.items():
            steps = job.get("steps", [])
            if not steps:
                report("warn", f"{path.name} 的 job「{job_name}」没有 steps")
                continue
            body = json.dumps(steps, ensure_ascii=False)
            # tauri-codegen 编译期要读 frontendDist，所以 Rust job 必须先建前端
            has_node_setup = "actions/setup-node" in body
            has_frontend_build = "npm run build" in body or "npm ci" in body
            if "cargo check" in body or "tauri-action" in body:
                if not has_node_setup:
                    report("fail", f"job「{job_name}」编译 Rust 但缺 actions/setup-node")
                if "cargo check" in body and not has_frontend_build:
                    report("fail", f"job「{job_name}」cargo check 前未构建前端，必然失败")
                if has_node_setup and has_frontend_build:
                    report("ok", f"job「{job_name}」已按「先建前端再编译」排序")


def check_lockfile() -> None:
    section("依赖锁定")
    if (ROOT / "package-lock.json").is_file():
        report("ok", "package-lock.json 已提交（npm ci 需要）")
    else:
        report("fail", "缺少 package-lock.json，CI 里的 npm ci 会失败")


# ------------------------------------------------------------- 前后端对齐校验

def find_commands_called() -> set[str]:
    """前端 invoke('x') 调用的命令名。"""
    names: set[str] = set()
    pattern = re.compile(r"invoke\s*(?:<[^>]*>)?\s*\(\s*['\"]([a-zA-Z0-9_]+)['\"]")
    for path in (ROOT / "src").rglob("*.ts*"):
        names |= set(pattern.findall(path.read_text(encoding="utf-8")))
    return names


def find_commands_registered() -> set[str]:
    """lib.rs 里 generate_handler![...] 注册的命令名（去模块前缀）。"""
    lib = (TAURI / "src" / "lib.rs").read_text(encoding="utf-8")
    match = re.search(r"generate_handler!\s*\[(.*?)\]", lib, re.S)
    if not match:
        return set()
    names = set()
    for raw in re.findall(r"[A-Za-z_][A-Za-z0-9_:]*", match.group(1)):
        names.add(raw.split("::")[-1])
    return names


def check_command_alignment() -> None:
    section("前后端命令对齐")
    called = find_commands_called()
    registered = find_commands_registered()
    report("ok", f"前端调用 {len(called)} 个：{', '.join(sorted(called))}")
    report("ok", f"后端注册 {len(registered)} 个")

    for name in sorted(called - registered):
        report("fail", f"前端调用了 invoke('{name}')，但后端未注册 —— 运行时才会报错")
    for name in sorted(registered - called):
        report("warn", f"后端注册了 {name}，前端从未调用")


def check_permissions(config: dict | None) -> None:
    section("capabilities 权限覆盖")
    cap = load_json(TAURI / "capabilities" / "default.json")
    if not isinstance(cap, dict):
        return
    perms = set(cap.get("permissions", []))
    if "core:default" not in perms:
        report("warn", "capabilities 未包含 core:default")

    # 前端调用的 window API -> 需要的权限
    frontend = "\n".join(
        p.read_text(encoding="utf-8") for p in (ROOT / "src").rglob("*.ts*")
    )
    used = {
        perm
        for api, perm in API_TO_PERMISSION.items()
        if re.search(r"\.\s*" + api + r"\s*\(", frontend)
    }
    if "data-tauri-drag-region" in frontend:
        used.add("core:window:allow-start-dragging")

    for perm in sorted(used):
        if perm in perms:
            report("ok", f"前端用到 -> {perm} 已授权")
        else:
            report("fail", f"前端调用了对应 API，但 capabilities 缺 {perm}")

    rust_src = "\n".join(p.read_text(encoding="utf-8") for p in (TAURI / "src").rglob("*.rs"))

    for perm in sorted(perms - used - {"core:default"}):
        hint = RUST_API_HINTS.get(perm)
        if hint and re.search(hint, rust_src):
            report(
                "warn",
                f"{perm} 仅 Rust 侧在用 —— Rust 调用不经 ACL，这条授权可移除",
            )
        else:
            report("warn", f"{perm} 前端与 Rust 均未检测到调用，疑似冗余")

    # capabilities 里的窗口 label 必须真实存在
    if config:
        labels = {w.get("label", "main") for w in config.get("app", {}).get("windows", [])}
        for label in cap.get("windows", []):
            if labels and label not in labels:
                report("fail", f"capabilities 指向窗口「{label}」，但 tauri.conf.json 中不存在")


# ---------------------------------------------------------------- schema 校验

def fetch_schema(url: str, cache_name: str) -> dict | None:
    CACHE.mkdir(exist_ok=True)
    cached = CACHE / cache_name
    if cached.is_file():
        try:
            return json.loads(cached.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            cached.unlink()
    try:
        # 不带 UA 会被 schema.tauri.app 以 403 拒绝
        req = urllib.request.Request(
            url, headers={"User-Agent": "clipnest-verify/1.0 (+local static check)"}
        )
        with urllib.request.urlopen(req, timeout=20) as resp:
            data = json.loads(resp.read().decode("utf-8"))
        cached.write_text(json.dumps(data), encoding="utf-8")
        return data
    except Exception as exc:  # noqa: BLE001 - 离线时降级即可
        report("warn", f"无法获取 schema（{exc}），跳过 {url}")
        return None


def check_schema() -> None:
    section("tauri.conf.json schema 校验")
    try:
        from jsonschema import Draft7Validator
    except ImportError:
        report("warn", "未安装 jsonschema，跳过")
        return

    schema = fetch_schema(TAURI_CONFIG_SCHEMA, "tauri-config-2.json")
    if not schema:
        return
    instance = load_json(TAURI / "tauri.conf.json")
    if instance is None:
        return

    errors = sorted(Draft7Validator(schema).iter_errors(instance), key=lambda e: list(e.path))
    if not errors:
        report("ok", "tauri.conf.json 通过官方 v2 schema 校验")
        return
    for err in errors[:15]:
        loc = "/".join(str(p) for p in err.path) or "(root)"
        report("fail", f"schema: {loc} -> {err.message}")
    if len(errors) > 15:
        report("fail", f"... 另有 {len(errors) - 15} 条 schema 错误")


def main() -> int:
    offline = "--no-net" in sys.argv
    print(f"ClipNest 静态校验  (root={ROOT})")

    config = check_config()
    check_lockfile()
    check_workflows()
    check_command_alignment()
    check_permissions(config)
    if not offline:
        check_schema()
    else:
        print("\n(已跳过联网 schema 校验)")

    print("\n" + "=" * 56)
    print(f"通过 {len(ok)}  |  警告 {len(warn)}  |  失败 {len(fail)}")
    if fail:
        print("\n失败项：")
        for item in fail:
            print("  - " + item)
    return 1 if fail else 0


if __name__ == "__main__":
    sys.exit(main())
