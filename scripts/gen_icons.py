#!/usr/bin/env python3
"""从单张 1024+ 源图生成 Conch 的 macOS / Windows 应用图标。

源图为 AI 生成的「黑底 + 居中圆角玻璃砖」。本脚本:
  1. 阈值找内容包围盒,裁出玻璃砖并补成正方形;
  2. 生成两套母版(透明圆角):
       - macOS:按 Apple 网格留白(砖≈80%)、连续大圆角;
       - Windows:近满幅(砖≈96%)、适中圆角;
  3. 扇出到 macOS AppIcon.appiconset 的各尺寸 PNG + Windows app_icon.ico。

Pillow 的 ICO 写入会丢掉除 16x16 以外的尺寸(曾经产出 899 字节坏文件),
Windows 任务栏因此回落到空白文档图标。这里用手写 ICO:
小尺寸用 32-bit BMP,256 用 PNG。

用法:
  python3 scripts/gen_icons.py [源图路径]
  python3 scripts/gen_icons.py --from-branding   # 只用已有 Windows 母版重做 ico
"""
from __future__ import annotations

import io
import struct
import sys
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
BRAND_DIR = ROOT / "desktop/assets/branding"
MAC_SET = ROOT / "desktop/macos/Runner/Assets.xcassets/AppIcon.appiconset"
WIN_ICO = ROOT / "desktop/windows/runner/resources/app_icon.ico"
WIN_MASTER_PNG = BRAND_DIR / "conch_icon_windows.png"

MAC_SIZES = [16, 32, 64, 128, 256, 512, 1024]
# 20/40 覆盖 125%/150% 任务栏缩放;256 给资源管理器大图标。
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]


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


def _png_bytes(im: Image.Image) -> bytes:
    buf = io.BytesIO()
    im.save(buf, format="PNG")
    return buf.getvalue()


def _bmp_icon_bytes(im: Image.Image) -> bytes:
    """ICO 用的 32-bit DIB: BITMAPINFOHEADER + 自下而上 BGRA XOR + AND mask。"""
    im = im.convert("RGBA")
    w, h = im.size
    raw = im.tobytes("raw", "BGRA")
    stride = w * 4
    xor = bytearray()
    for y in range(h - 1, -1, -1):
        xor += raw[y * stride:(y + 1) * stride]
    and_stride = ((w + 31) // 32) * 4
    and_mask = bytes(and_stride * h)
    header = struct.pack(
        "<IiiHHIIiiII",
        40,
        w,
        h * 2,
        1,
        32,
        0,
        len(xor) + len(and_mask),
        0,
        0,
        0,
        0,
    )
    return header + xor + and_mask


def write_windows_ico(win_master: Image.Image, dest: Path = WIN_ICO) -> None:
    """写带多尺寸的合法 ICO,避免 Pillow ICO 只留下 16x16。"""
    dest.parent.mkdir(parents=True, exist_ok=True)
    images = [
        win_master.resize((s, s), Image.LANCZOS).convert("RGBA")
        for s in ICO_SIZES
    ]
    entries: list[tuple[int, int, bytes]] = []
    for im in images:
        w, h = im.size
        data = _png_bytes(im) if max(w, h) >= 256 else _bmp_icon_bytes(im)
        entries.append((w, h, data))

    offset = 6 + 16 * len(entries)
    out = bytearray(struct.pack("<HHH", 0, 1, len(entries)))
    blobs = bytearray()
    for w, h, data in entries:
        out += struct.pack(
            "<BBBBHHII",
            0 if w >= 256 else w,
            0 if h >= 256 else h,
            0,
            0,
            1,
            32,
            len(data),
            offset,
        )
        offset += len(data)
        blobs += data
    dest.write_bytes(out + blobs)
    print(f"windows ico updated: {dest} ({dest.stat().st_size} bytes, {len(entries)} sizes)")


def resolve_source() -> Path | None:
    args = [a for a in sys.argv[1:] if not a.startswith("-")]
    if args:
        return Path(args[0])
    default = (
        Path.home()
        / ".cursor/projects/Users-shcodegojo-Project/assets/conch_icon.png"
    )
    if default.exists():
        return default
    return None


def main() -> None:
    from_branding = "--from-branding" in sys.argv
    src_path = resolve_source()

    if from_branding or src_path is None:
        if not WIN_MASTER_PNG.exists():
            sys.exit(
                "source image not found, and "
                f"{WIN_MASTER_PNG} is missing (cannot rebuild ico)"
            )
        print(f"rebuilding ICO from {WIN_MASTER_PNG}")
        write_windows_ico(Image.open(WIN_MASTER_PNG).convert("RGBA"))
        return

    if not src_path.exists():
        sys.exit(f"source image not found: {src_path}")

    BRAND_DIR.mkdir(parents=True, exist_ok=True)
    src = Image.open(src_path).convert("RGB")
    square = crop_tile(src)

    mac_master = rounded(square, 1024, margin_frac=0.098, radius_frac=0.224)
    win_master = rounded(square, 1024, margin_frac=0.012, radius_frac=0.16)
    mac_master.save(BRAND_DIR / "conch_icon_macos.png")
    win_master.save(WIN_MASTER_PNG)
    square.save(BRAND_DIR / "conch_icon_source_square.png")
    print(f"masters -> {BRAND_DIR}")

    if MAC_SET.exists():
        for size in MAC_SIZES:
            img = mac_master.resize((size, size), Image.LANCZOS)
            img.save(MAC_SET / f"app_icon_{size}.png")
        print(f"macOS appiconset updated: {MAC_SET}")

    write_windows_ico(win_master)


if __name__ == "__main__":
    main()
