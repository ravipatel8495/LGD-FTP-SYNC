#!/usr/bin/env python3
"""Build 24-bit BMPs for the Windows NSIS installer from the app icon."""
from pathlib import Path
from struct import pack, unpack_from
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1] / "src-tauri"
ICON = ROOT / "icons" / "icon.png"
OUT = ROOT / "windows"


def sips_bmp(src: Path, w: int, h: int, dest: Path) -> None:
    subprocess.check_call(
        ["sips", "-z", str(h), str(w), str(src), "--out", str(dest), "-s", "format", "bmp"],
        stdout=subprocess.DEVNULL,
    )


def read_bmp(path: Path):
    data = path.read_bytes()
    if data[:2] != b"BM":
        raise SystemExit(f"not a BMP: {path}")
    pixel_off = unpack_from("<I", data, 10)[0]
    width = unpack_from("<i", data, 18)[0]
    height = unpack_from("<i", data, 22)[0]
    bpp = unpack_from("<H", data, 28)[0]
    bottom_up = height > 0
    height = abs(height)
    row_raw = width * (bpp // 8)
    stride = (row_raw + 3) & ~3
    pixels = []
    for y in range(height):
        src_y = y if not bottom_up else (height - 1 - y)
        start = pixel_off + src_y * stride
        row = data[start : start + row_raw]
        pix = []
        if bpp == 32:
            for i in range(0, row_raw, 4):
                b, g, r, a = row[i : i + 4]
                pix.append((r, g, b, a))
        elif bpp == 24:
            for i in range(0, row_raw, 3):
                b, g, r = row[i : i + 3]
                pix.append((r, g, b, 255))
        else:
            raise SystemExit(f"unsupported bpp {bpp}")
        pixels.append(pix)
    return width, height, pixels


def write_bmp24(path: Path, pixels, bg):
    h = len(pixels)
    w = len(pixels[0])
    stride = (w * 3 + 3) & ~3
    pixel_off = 54
    size = pixel_off + stride * h
    out = bytearray()
    out += b"BM"
    out += pack("<IHHI", size, 0, 0, pixel_off)
    out += pack("<IiiHHIIiiII", 40, w, h, 1, 24, 0, stride * h, 2835, 2835, 0, 0)
    for y in range(h - 1, -1, -1):
        row = bytearray()
        for r, g, b, a in pixels[y]:
            if a < 255:
                t = a / 255.0
                r = int(r * t + bg[0] * (1 - t))
                g = int(g * t + bg[1] * (1 - t))
                b = int(b * t + bg[2] * (1 - t))
            row += bytes((b, g, r))
        row += b"\x00" * (stride - w * 3)
        out += row
    path.write_bytes(out)
    print(f"wrote {path} {w}x{h} ({len(out)} bytes)")


def solid(w, h, color):
    r, g, b = color
    return [[(r, g, b, 255)] * w for _ in range(h)]


def blit(dst, src, x, y):
    sh = len(src)
    sw = len(src[0])
    dh = len(dst)
    dw = len(dst[0])
    for j in range(sh):
        yy = y + j
        if yy < 0 or yy >= dh:
            continue
        for i in range(sw):
            xx = x + i
            if xx < 0 or xx >= dw:
                continue
            dst[yy][xx] = src[j][i]


def main():
    OUT.mkdir(exist_ok=True)
    cream = (245, 242, 236)
    teal = (20, 69, 75)
    gold = (217, 160, 43)
    with tempfile.TemporaryDirectory() as td:
        td = Path(td)
        icon48 = td / "i48.bmp"
        icon120 = td / "i120.bmp"
        sips_bmp(ICON, 48, 48, icon48)
        sips_bmp(ICON, 120, 120, icon120)
        _, _, i48 = read_bmp(icon48)
        _, _, i120 = read_bmp(icon120)

    header = solid(150, 57, cream)
    blit(header, i48, 6, (57 - 48) // 2)
    write_bmp24(OUT / "nsis-header.bmp", header, cream)

    sidebar = solid(164, 314, teal)
    for y in range(8):
        for x in range(164):
            sidebar[y][x] = (*gold, 255)
    blit(sidebar, i120, (164 - 120) // 2, 36)
    write_bmp24(OUT / "nsis-sidebar.bmp", sidebar, teal)


if __name__ == "__main__":
    main()
