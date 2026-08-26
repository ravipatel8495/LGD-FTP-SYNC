#!/usr/bin/env python3
"""Draw a 660x400 DMG background that matches Finder window + icon slots."""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "build" / "dmg-background.png"
WORDMARK = ROOT / "src" / "assets" / "wordmark.png"

W, H = 660, 400
BG = (246, 244, 239)
TEAL = (20, 70, 90)
MUTED = (90, 90, 90)
GOLD = (201, 148, 32)
GOLD_DARK = (168, 118, 18)

img = Image.new("RGB", (W, H), BG)
draw = ImageDraw.Draw(img)

def font(size, bold=False):
    candidates = [
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf" if bold else "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial Bold.ttf" if bold else "/Library/Fonts/Arial.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
    ]
    for path in candidates:
        try:
            return ImageFont.truetype(path, size)
        except OSError:
            continue
    return ImageFont.load_default()

# Wordmark
if WORDMARK.exists():
    mark = Image.open(WORDMARK).convert("RGBA")
    mark.thumbnail((220, 48), Image.Resampling.LANCZOS)
    img.paste(mark, ((W - mark.width) // 2, 18), mark)
else:
    draw.text((W // 2, 28), "AUGMONT", fill=TEAL, font=font(22, True), anchor="mt")

draw.text((W // 2, 78), "LGD FTP DATA SYNC  —  Installation", fill=TEAL, font=font(16, True), anchor="mt")
draw.text((W // 2, 108), "Step 1:  Drag the app icon onto the Applications folder  →", fill=MUTED, font=font(13), anchor="mt")
draw.text((W // 2, 130), "Step 2:  Open it from Launchpad or the Applications folder", fill=MUTED, font=font(13), anchor="mt")

# Gold arrow between the two icon slots (centers at 180,250 and 480,250)
cy = 250
x1, x2 = 250, 410
draw.line([(x1, cy), (x2 - 18, cy)], fill=GOLD, width=8)
draw.polygon([(x2 - 28, cy - 16), (x2, cy), (x2 - 28, cy + 16)], fill=GOLD_DARK)

draw.text(
    (W // 2, H - 28),
    "First open: right-click the app  →  Open  →  Open",
    fill=(150, 150, 150),
    font=font(11),
    anchor="mt",
)

img.save(OUT, "PNG")
print(f"wrote {OUT} {img.size}")
