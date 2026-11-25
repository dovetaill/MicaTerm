#!/usr/bin/env python3
"""Deterministic SVG -> PNG matrix & multi-frame Windows ICO generator for Mica Term GPUI."""

from pathlib import Path
import shutil
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
ICONS_DIR = ROOT / "assets" / "icons"
PNG_DIR = ICONS_DIR / "png"
WIN_DIR = ICONS_DIR / "windows"

TASKBAR_SVG = ICONS_DIR / "mica-term-gpui-taskbar.svg"
APP_SVG = ICONS_DIR / "mica-term-gpui-app.svg"
MARK_SVG = ICONS_DIR / "mica-term-gpui-mark.svg"

ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]
SMALL_CUTOFF = 32


def render_png(svg_path: Path, out_png: Path, px_size: int) -> bytes:
    subprocess.run(
        [
            "rsvg-convert",
            "-w",
            str(px_size),
            "-h",
            str(px_size),
            "-o",
            str(out_png),
            str(svg_path),
        ],
        check=True,
    )
    data = out_png.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", f"invalid PNG header: {out_png}"
    width, height = struct.unpack(">II", data[16:24])
    assert (width, height) == (px_size, px_size), f"unexpected PNG size {width}x{height} in {out_png}"
    return data


def build_ico(frames: list[tuple[int, bytes]], out_ico: Path) -> None:
    count = len(frames)
    header = struct.pack("<HHH", 0, 1, count)
    offset = 6 + 16 * count
    entries = bytearray()
    payload = bytearray()

    for dim, png_bytes in frames:
        w_byte = 0 if dim >= 256 else dim
        h_byte = 0 if dim >= 256 else dim
        entries += struct.pack(
            "<BBBBHHII",
            w_byte,
            h_byte,
            0,
            0,
            1,
            32,
            len(png_bytes),
            offset,
        )
        payload += png_bytes
        offset += len(png_bytes)

    out_ico.write_bytes(bytes(header) + bytes(entries) + bytes(payload))


def main() -> int:
    if not shutil.which("rsvg-convert"):
        print("error: rsvg-convert is required to rasterize SVG icons", file=sys.stderr)
        return 1

    PNG_DIR.mkdir(parents=True, exist_ok=True)
    WIN_DIR.mkdir(parents=True, exist_ok=True)

    ico_frames: list[tuple[int, bytes]] = []
    for dim in ICO_SIZES:
        src = TASKBAR_SVG if dim <= SMALL_CUTOFF else APP_SVG
        out_png = PNG_DIR / f"mica-term-gpui-{dim}.png"
        ico_frames.append((dim, render_png(src, out_png, dim)))

    render_png(MARK_SVG, PNG_DIR / "mica-term-gpui-mark-64.png", 64)

    ico_path = WIN_DIR / "mica-term-gpui.ico"
    build_ico(ico_frames, ico_path)

    raw = ico_path.read_bytes()
    reserved, ico_type, count = struct.unpack("<HHH", raw[:6])
    assert (reserved, ico_type, count) == (0, 1, len(ICO_SIZES))
    print(f"Generated {len(ICO_SIZES)} PNG frames + mark PNG + {ico_path.relative_to(ROOT)} ({len(raw)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
