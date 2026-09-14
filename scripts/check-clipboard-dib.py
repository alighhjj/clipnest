#!/usr/bin/env python3
"""clipboard.rs 里 CF_DIB 解析逻辑的离线对照测试。

本机没有 Rust 工具链，所以 clipboard.rs 的编译验证交给 CI。但「头部偏移读错」
这类问题编译器不会报，只会在运行时把图弄花或者弄成全透明，因此在本地用同一套
偏移规则复算一遍，再让一个独立解码器（Pillow）交叉验证，是性价比最高的保险。

被测的实现约定（见 src-tauri/src/clipboard.rs 的 read_legacy_cf_dib /
tweak_cf_dib_header）：

    biSize          @0    u32
    biBitCount      @14   u16
    biCompression   @16   u32
    biRedMask       @40   u32   仅当 biSize >= 56 时存在
    biGreenMask     @44   u32
    biBlueMask      @48   u32
    biAlphaMask     @52   u32

用法：
    python scripts/check-clipboard-dib.py
"""

from __future__ import annotations

import io
import struct
import sys

try:
    from PIL import Image
except ImportError:  # pragma: no cover - 环境缺失时明确报错而不是静默跳过
    print("需要 Pillow：请用带 Pillow 的 Python 运行本脚本")
    sys.exit(2)

BI_RGB = 0
BI_BITFIELDS = 3
ALPHA_MASK_FULL = 0xFF00_0000

BITMAPINFOHEADER_SIZE = 40
BITMAPV5HEADER_SIZE = 124

# 采样图里用到的几个像素，写成 (b, g, r, a)
RED = (0, 0, 255, 255)
HALF_BLUE = (255, 0, 0, 128)
GREEN = (0, 255, 0, 255)
BLACK = (0, 0, 0, 255)

results: list[tuple[bool, str]] = []


def check(condition: bool, label: str) -> bool:
    results.append((bool(condition), label))
    print(f"  {'OK  ' if condition else 'FAIL'} {label}")
    return bool(condition)


# --------------------------------------------------------------------- 被测逻辑

def read_u32(raw: bytes | bytearray, at: int) -> int:
    return struct.unpack_from("<I", raw, at)[0]


def write_u32(raw: bytearray, at: int, value: int) -> None:
    struct.pack_into("<I", raw, at, value)


def tweak_cf_dib_header(raw: bytearray, bit_count: int, compression: int) -> bool:
    """与 Rust 版 tweak_cf_dib_header 逐行对应，返回是否做了改写。"""
    if bit_count != 32 or compression != BI_RGB:
        return False
    if read_u32(raw, 52) != ALPHA_MASK_FULL:
        return False

    if read_u32(raw, 40) == 0 and read_u32(raw, 44) == 0 and read_u32(raw, 48) == 0:
        write_u32(raw, 40, 0x00FF_0000)
        write_u32(raw, 44, 0x0000_FF00)
        write_u32(raw, 48, 0x0000_00FF)
    write_u32(raw, 16, BI_BITFIELDS)
    return True


def may_read_masks(dib: bytes, bit_count: int, compression: int) -> bool:
    """Rust 侧调用 tweak 的门槛：biSize 与缓冲长度都得够到掩码区。"""
    bi_size = read_u32(dib, 0)
    del bit_count, compression  # 门槛只看头大小，参数保留是为了贴近调用点
    return bi_size >= 56 and len(dib) >= 56


def force_opaque_if_fully_transparent(rgba: bytearray) -> bool:
    """与 Rust 版「全透明兜底」对应，返回是否被改写。"""
    if all(rgba[i] == 0 for i in range(3, len(rgba), 4)):
        for i in range(3, len(rgba), 4):
            rgba[i] = 255
        return True
    return False


# --------------------------------------------------------------------- 造数据

def sample_rows(width: int, height: int) -> list[list[tuple[int, int, int, int]]]:
    """自顶向下返回像素行。

    只在四个角放固定颜色，方便定位：
        (0, 0)              纯红
        (width-1, 0)        半透明蓝
        (0, height-1)       纯绿
        (width-1, height-1) 不透明黑（用于让偏移 52 落在「长得像掩码」的字节上）
    其余为不透明黑。
    """
    rows = []
    for y in range(height):
        row = []
        for x in range(width):
            if (x, y) == (0, 0):
                row.append(RED)
            elif (x, y) == (width - 1, 0):
                row.append(HALF_BLUE)
            elif (x, y) == (0, height - 1):
                row.append(GREEN)
            else:
                row.append(BLACK)
        rows.append(row)
    return rows


def build_dib(
    width: int,
    height: int,
    rows_top_down: list[list[tuple[int, int, int, int]]],
    *,
    header_size: int = BITMAPV5HEADER_SIZE,
    bit_count: int = 32,
    compression: int = BI_RGB,
    rgb_masks: tuple[int, int, int] = (0, 0, 0),
    alpha_mask: int = 0,
) -> bytearray:
    """按 Windows 剪贴板里 CF_DIB 的布局拼数据（无文件头，像素自底向上）。"""
    header = bytearray(header_size)
    write_u32(header, 0, header_size)
    struct.pack_into("<i", header, 4, width)
    struct.pack_into("<i", header, 8, height)
    struct.pack_into("<H", header, 12, 1)
    struct.pack_into("<H", header, 14, bit_count)
    write_u32(header, 16, compression)
    write_u32(header, 20, width * height * (bit_count // 8))

    if header_size >= 56:
        write_u32(header, 40, rgb_masks[0])
        write_u32(header, 44, rgb_masks[1])
        write_u32(header, 48, rgb_masks[2])
        write_u32(header, 52, alpha_mask)

    body = bytearray()
    for row in reversed(rows_top_down):  # BMP 像素自底向上
        for b, g, r, a in row:
            body += bytes((b, g, r, a))
    return header + body


def as_bmp_file(dib: bytearray) -> bytes:
    """给 DIB 补一个 BITMAPFILEHEADER，好让 Pillow 能直接解。"""
    header_size = read_u32(dib, 0)
    file_header = bytearray(14)
    file_header[0:2] = b"BM"
    write_u32(file_header, 2, 14 + len(dib))
    write_u32(file_header, 10, 14 + header_size)
    return bytes(file_header) + bytes(dib)


def decode(dib: bytearray) -> Image.Image:
    return Image.open(io.BytesIO(as_bmp_file(dib)))


def alpha_values(image: Image.Image) -> set[int]:
    return set(image.getchannel("A").tobytes())


# --------------------------------------------------------------------- 测试

def test_offsets() -> None:
    print("\n=== 头部字段偏移 ===")
    dib = build_dib(
        2,
        2,
        sample_rows(2, 2),
        rgb_masks=(0x001F_0000, 0x0000_0F00, 0x0000_001F),
        alpha_mask=ALPHA_MASK_FULL,
    )
    check(read_u32(dib, 0) == 124, "biSize @0 = 124")
    check(struct.unpack_from("<H", dib, 14)[0] == 32, "biBitCount @14 = 32")
    check(read_u32(dib, 16) == BI_RGB, "biCompression @16 = BI_RGB")
    check(read_u32(dib, 40) == 0x001F_0000, "biRedMask @40")
    check(read_u32(dib, 44) == 0x0000_0F00, "biGreenMask @44")
    check(read_u32(dib, 48) == 0x0000_001F, "biBlueMask @48")
    check(read_u32(dib, 52) == ALPHA_MASK_FULL, "biAlphaMask @52")


def test_chrome_style_alpha_preserved() -> None:
    """Chrome 系应用：V5 头 + BI_RGB + 有效 alpha 掩码 + RGB 掩码留空。"""
    print("\n=== 场景 A：V5 头 / BI_RGB / alpha 掩码有效（Chrome 系） ===")
    dib = build_dib(4, 4, sample_rows(4, 4), alpha_mask=ALPHA_MASK_FULL, rgb_masks=(0, 0, 0))

    check(tweak_cf_dib_header(dib, 32, read_u32(dib, 16)), "tweak 生效")
    check(read_u32(dib, 16) == BI_BITFIELDS, "biCompression 改写为 BI_BITFIELDS")
    check(read_u32(dib, 40) == 0x00FF_0000, "补上 biRedMask")
    check(read_u32(dib, 44) == 0x0000_FF00, "补上 biGreenMask")
    check(read_u32(dib, 48) == 0x0000_00FF, "补上 biBlueMask")
    check(read_u32(dib, 52) == ALPHA_MASK_FULL, "biAlphaMask 保持不变")

    decoded = decode(dib).convert("RGBA")
    check(decoded.size == (4, 4), f"独立解码器解出 4x4（实际 {decoded.size}）")
    check(decoded.getpixel((0, 0)) == (255, 0, 0, 255), f"左上纯红，实际 {decoded.getpixel((0, 0))}")
    check(
        decoded.getpixel((3, 0)) == (0, 0, 255, 128),
        f"右上半透明蓝（alpha 未被丢掉），实际 {decoded.getpixel((3, 0))}",
    )
    check(decoded.getpixel((0, 3)) == (0, 255, 0, 255), f"左下纯绿，实际 {decoded.getpixel((0, 3))}")


def test_v5_without_alpha_mask() -> None:
    """V5 头但没声明 alpha 掩码：应保持不透明，不能被判成全透明。"""
    print("\n=== 场景 B：V5 头 / BI_RGB / 无 alpha 掩码 ===")
    dib = build_dib(4, 4, sample_rows(4, 4), alpha_mask=0, rgb_masks=(0, 0, 0))
    before = bytes(dib)

    check(not tweak_cf_dib_header(dib, 32, read_u32(dib, 16)), "tweak 不生效")
    check(bytes(dib) == before, "头部字节未被改写")

    decoded = decode(dib).convert("RGBA")
    check(decoded.size == (4, 4), "独立解码器解出 4x4")
    check(alpha_values(decoded) == {255}, f"alpha 全为 255（实际 {sorted(alpha_values(decoded))}）")


def test_40_byte_info_header() -> None:
    """40 字节 BITMAPINFOHEADER：偏移 52 落在像素上，绝不能当掩码读。"""
    print("\n=== 场景 C：40 字节 BITMAPINFOHEADER / 32bpp ===")
    # 右下角那个不透明黑 (0,0,0,255) 的字节序恰好是 ff 00 00 00，
    # 也就是从偏移 52 按小端读出来的 0xff000000 —— 长得很像一个 alpha 掩码。
    dib = build_dib(4, 4, sample_rows(4, 4), header_size=BITMAPINFOHEADER_SIZE, alpha_mask=0)

    check(read_u32(dib, 0) == BITMAPINFOHEADER_SIZE, "biSize = 40")
    check(read_u32(dib, 52) == ALPHA_MASK_FULL, "偏移 52 读到 0xff000000（其实是像素，像极了 alpha 掩码）")
    check(not may_read_masks(dib, 32, BI_RGB), "biSize=40 被门槛拦住，不会去读偏移 52")
    check(dib[40:44] == bytes(GREEN), "偏移 40 起确实是像素（左下角纯绿）")

    decoded = decode(dib).convert("RGBA")
    check(decoded.size == (4, 4), "独立解码器解出 4x4")
    check(decoded.getpixel((0, 0)) == (255, 0, 0, 255), f"左上纯红，实际 {decoded.getpixel((0, 0))}")
    check(alpha_values(decoded) == {255}, "未声明 alpha，整图不透明")


def test_fully_transparent_fallback() -> None:
    """全透明兜底：声明了 alpha 却全填 0 时，置为不透明。"""
    print("\n=== 场景 D：alpha 全为 0 的兜底 ===")
    rgba = bytearray()
    for _ in range(4):
        rgba += bytes((10, 20, 30, 0))

    check(force_opaque_if_fully_transparent(rgba), "触发改写")
    check({rgba[i] for i in range(3, len(rgba), 4)} == {255}, "alpha 全部置为 255")
    check(bytes(rgba[:3]) == bytes((10, 20, 30)), "颜色通道未被影响")

    mixed = bytearray()
    for i in range(4):
        mixed += bytes((10, 20, 30, 0 if i == 0 else 255))
    check(not force_opaque_if_fully_transparent(mixed), "只要有一个像素不透明就跳过")


def main() -> int:
    test_offsets()
    test_chrome_style_alpha_preserved()
    test_v5_without_alpha_mask()
    test_40_byte_info_header()
    test_fully_transparent_fallback()

    failed = [label for passed, label in results if not passed]
    print("\n" + "=" * 56)
    print(f"通过 {len(results) - len(failed)} / {len(results)}")
    if failed:
        print("\n失败项：")
        for label in failed:
            print("  - " + label)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
