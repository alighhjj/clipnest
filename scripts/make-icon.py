"""生成 ClipNest 的应用图标源图。

用法：
    python scripts/make-icon.py [输出路径]

默认输出 app-icon.png（1024×1024），随后用 `npm run tauri icon app-icon.png`
派生出各平台所需的图标文件。想换配色改下面的 COLORS 即可。
"""

from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 1024
SUPERSAMPLE = 4
S = SIZE * SUPERSAMPLE

# 背景渐变：上 -> 下
GRADIENT_TOP = (0x4E, 0x8C, 0xFF)
GRADIENT_BOTTOM = (0x28, 0x54, 0xD2)
GLYPH = (255, 255, 255, 255)


def r(value: float) -> float:
    return value * S


def make_gradient() -> Image.Image:
    top_r, top_g, top_b = GRADIENT_TOP
    bottom_r, bottom_g, bottom_b = GRADIENT_BOTTOM
    gradient = Image.new("RGB", (1, S))
    pixels = gradient.load()
    for y in range(S):
        t = y / max(S - 1, 1)
        # 略带对角感：用 gamma 让上半段更亮
        t = t**0.85
        pixels[0, y] = (
            round(top_r + (bottom_r - top_r) * t),
            round(top_g + (bottom_g - top_g) * t),
            round(top_b + (bottom_b - top_b) * t),
        )
    return gradient.resize((S, S), Image.NEAREST).convert("RGBA")


def build() -> Image.Image:
    canvas = Image.new("RGBA", (S, S), (0, 0, 0, 0))

    # 1) 圆角方块 + 渐变
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, S - 1, S - 1), radius=r(0.225), fill=255
    )
    canvas.paste(make_gradient(), (0, 0), mask)

    # 2) 白色剪贴板字形
    draw = ImageDraw.Draw(canvas)
    stroke = r(0.040)

    draw.rounded_rectangle(
        (r(0.300), r(0.230), r(0.700), r(0.800)),
        radius=r(0.062),
        outline=GLYPH,
        width=round(stroke),
    )

    # 顶部夹子
    draw.rounded_rectangle(
        (r(0.418), r(0.162), r(0.582), r(0.286)),
        radius=r(0.036),
        fill=GLYPH,
    )

    # 两条文本线
    line_width = round(r(0.034))
    draw.line(
        [(r(0.382), r(0.455)), (r(0.618), r(0.455))],
        fill=GLYPH,
        width=line_width,
    )
    draw.line(
        [(r(0.382), r(0.585)), (r(0.545), r(0.585))],
        fill=GLYPH,
        width=line_width,
    )

    # 圆头收口
    radius = line_width / 2
    for x, y in ((0.382, 0.455), (0.618, 0.455), (0.382, 0.585), (0.545, 0.585)):
        draw.ellipse(
            (r(x) - radius, r(y) - radius, r(x) + radius, r(y) + radius), fill=GLYPH
        )

    return canvas.resize((SIZE, SIZE), Image.LANCZOS)


def main() -> None:
    target = Path(sys.argv[1] if len(sys.argv) > 1 else "app-icon.png")
    target.parent.mkdir(parents=True, exist_ok=True)
    build().save(target)
    print(f"已生成 {target.resolve()}")


if __name__ == "__main__":
    main()
