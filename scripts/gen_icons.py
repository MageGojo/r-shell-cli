#!/usr/bin/env python3
"""从单张 1024+ 源图生成 Conch 的 macOS / Windows 应用图标。

源图为 AI 生成的「黑底 + 居中圆角玻璃砖」。本脚本:
  1. 阈值找内容包围盒,裁出玻璃砖并补成正方形;
  2. 生成两套母版(透明圆角):
       - macOS:按 Apple 网格留白(砖≈80%)、连续大圆角;
       - Windows:近满幅(砖≈96%)、适中圆角;
  3. 扇出到 macOS AppIcon.appiconset 的各尺寸 PNG + Windows app_icon.ico。

用法: python3 scripts/gen_icons.py [源图路径]
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
SRC = Path(sys.argv[1]) if len(sys.argv) > 1 else (
    Path.home()
    / ".cursor/projects/Users-shcodegojo-Project/assets/conch_icon.png"
)
BRAND_DIR = ROOT / "desktop/assets/branding"
MAC_SET = ROOT / "desktop/macos/Runner/Assets.xcassets/AppIcon.appiconset"
WIN_ICO = ROOT / "desktop/windows/runner/resources/app_icon.ico"

MAC_SIZES = [16, 32, 64, 128, 256, 512, 1024]
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]


def crop_tile(src: Image.Image) -> Image.Image:
    """阈值找包围盒裁出玻璃砖,再补成正方形(居中)。"""
    gray = src.convert("L")
    mask = gray.point(lambda p: 255 if p > 12 else 0)
    bbox = mask.getbbox()
    print(f"detected content bbox: {bbox} (src={src.size})")
    tile = src.crop(bbox)
    w, h = tile.size
    s = max(w, h)
    square = Image.new("RGB", (s, s), (0, 0, 0))
    square.paste(tile, ((s - w) // 2, (s - h) // 2))
    return square


def rounded(square: Image.Image, canvas: int, margin_frac: float,
            radius_frac: float) -> Image.Image:
    """把方形砖放到透明画布上,套圆角 alpha;margin/radius 为占比。"""
    inner = int(round(canvas * (1 - 2 * margin_frac)))
    tile = square.resize((inner, inner), Image.LANCZOS).convert("RGBA")
    alpha = Image.new("L", (inner, inner), 0)
    draw = ImageDraw.Draw(alpha)
    radius = int(round(inner * radius_frac))
    draw.rounded_rectangle([0, 0, inner - 1, inner - 1], radius=radius, fill=255)
    tile.putalpha(alpha)
    out = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    off = (canvas - inner) // 2
    out.paste(tile, (off, off), tile)
    return out


def main() -> None:
    if not SRC.exists():
        sys.exit(f"source image not found: {SRC}")
    BRAND_DIR.mkdir(parents=True, exist_ok=True)

    src = Image.open(SRC).convert("RGB")
    square = crop_tile(src)

    mac_master = rounded(square, 1024, margin_frac=0.098, radius_frac=0.224)
    win_master = rounded(square, 1024, margin_frac=0.012, radius_frac=0.16)
    mac_master.save(BRAND_DIR / "conch_icon_macos.png")
    win_master.save(BRAND_DIR / "conch_icon_windows.png")
    square.save(BRAND_DIR / "conch_icon_source_square.png")
    print(f"masters -> {BRAND_DIR}")

    for size in MAC_SIZES:
        img = mac_master.resize((size, size), Image.LANCZOS)
        img.save(MAC_SET / f"app_icon_{size}.png")
    print(f"macOS appiconset updated: {MAC_SET}")

    frames = [win_master.resize((s, s), Image.LANCZOS) for s in ICO_SIZES]
    frames[0].save(
        WIN_ICO, format="ICO",
        sizes=[(s, s) for s in ICO_SIZES],
        append_images=frames[1:],
    )
    print(f"windows ico updated: {WIN_ICO}")


if __name__ == "__main__":
    main()
