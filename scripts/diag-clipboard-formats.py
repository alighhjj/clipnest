"""ClipNest 剪贴板格式诊断工具（Windows）

用途：当「复制了图片但没被记录」时，先复制一次出问题的那张图，
再运行本脚本，就能看到当时剪贴板里到底有哪几种格式，
从而判断是不是格式支持范围的问题。

用法：
    python scripts/diag-clipboard-formats.py

不需要任何第三方依赖（仅标准库 ctypes）。
"""

import ctypes

from ctypes import wintypes

user32 = ctypes.WinDLL("user32", use_last_error=True)

user32.OpenClipboard.argtypes = [wintypes.HWND]
user32.OpenClipboard.restype = wintypes.BOOL
user32.CloseClipboard.restype = wintypes.BOOL
user32.EnumClipboardFormats.argtypes = [wintypes.UINT]
user32.EnumClipboardFormats.restype = wintypes.UINT
user32.GetClipboardFormatNameW.argtypes = [wintypes.UINT, wintypes.LPWSTR, ctypes.c_int]
user32.GetClipboardFormatNameW.restype = ctypes.c_int
user32.RegisterClipboardFormatW.argtypes = [wintypes.LPCWSTR]
user32.RegisterClipboardFormatW.restype = wintypes.UINT
user32.IsClipboardFormatAvailable.argtypes = [wintypes.UINT]
user32.IsClipboardFormatAvailable.restype = wintypes.BOOL

CF_BITMAP = 2
CF_DIB = 8
CF_UNICODETEXT = 13
CF_DIBV5 = 17
CF_HDROP = 15

# arboard 3.6.1 在 Windows 上实际读取的图片格式（见 crates.io 源码
# arboard-3.6.1/src/platform/windows.rs 的 Get::image）
ARBOARD_READS = ("PNG", "CF_DIBV5")
# 已知会被 arboard 忽略、但在真实应用中很常见的格式
ARBOARD_IGNORES = ("CF_DIB", "CF_BITMAP", "CF_HDROP")


def main() -> None:
    png_id = user32.RegisterClipboardFormatW("PNG")

    if not user32.OpenClipboard(None):
        print("无法打开剪贴板（可能被其它程序占用），请稍后重试。")
        return

    present = []
    fmt = 0
    while True:
        fmt = user32.EnumClipboardFormats(fmt)
        if fmt == 0:
            break
        buf = ctypes.create_unicode_buffer(256)
        n = user32.GetClipboardFormatNameW(fmt, buf, 256)
        present.append((fmt, buf.value if n > 0 else "标准格式"))

    print("当前剪贴板包含的格式：")
    for num, name in present:
        print(f"  {num:>5}  {name}")

    def avail(label: str, fid: int) -> bool:
        ok = bool(user32.IsClipboardFormatAvailable(fid))
        print(f"  {label:<14} {'可用' if ok else '不可用'}")
        return ok

    print("\n关键格式可用性：")
    has_png = avail("PNG", png_id)
    has_dibv5 = avail("CF_DIBV5", CF_DIBV5)
    has_dib = avail("CF_DIB", CF_DIB)
    has_bitmap = avail("CF_BITMAP", CF_BITMAP)
    has_hdrop = avail("CF_HDROP", CF_HDROP)
    has_text = avail("CF_UNICODETEXT", CF_UNICODETEXT)

    print("\n结论：")
    if has_hdrop and not (has_png or has_dibv5 or has_dib):
        print("  · 这是「复制的文件」而非「复制的图片」（CF_HDROP）。")
        print("    ClipNest 目前不记录文件列表，所以不会有图片记录。")
    elif has_png or has_dibv5:
        print("  · 含 PNG / CF_DIBV5，arboard 3.6.1 可读，理应被记录。")
        print("    若仍未记录，请查看应用日志中是否有「保存图片失败」。")
    elif has_dib or has_bitmap:
        print("  · 只有 CF_DIB / CF_BITMAP（典型来自截图工具、部分原生程序）。")
        print("    arboard 3.6.1 的 Get::image 不读这两者 → 图片不会被记录。")
        print("    这是当前实现的格式覆盖缺口。")
    elif has_text:
        print("  · 剪贴板里只有文本，没有图片数据。")
    else:
        print("  · 未发现可识别的图片或文本数据。")

    user32.CloseClipboard()


if __name__ == "__main__":
    main()
