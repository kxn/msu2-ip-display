from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parents[2]
ASSET_DIR = ROOT / "flasher" / "src-tauri" / "assets"
MOCKUP_DIR = ROOT / "docs" / "mockups"

WIDTH = 160
HEIGHT = 80
IMAGE_BYTES = WIDTH * HEIGHT * 2
FRAME_COUNT = 36

BASE_ASSET = ASSET_DIR / "offline.rgb565be"
BLANK_ASSET = ASSET_DIR / "ip_bg.rgb565be"
ANIMATION_ASSET = ASSET_DIR / "offline_animation.rgb565be"
WAITING_ASSET = ASSET_DIR / "waiting_to_flash.rgb565be"
CONTACT_SHEET = MOCKUP_DIR / "msu2-offline-animation-contact-sheet.png"
WAITING_MOCKUP = MOCKUP_DIR / "msu2-waiting-to-flash.png"
FONT_PATH = Path("C:/Windows/Fonts/msyhbd.ttc")

# Firmware plays the 36 offline frames at about 100ms per frame.
# This yields two hard blink cycles: 0.9s visible, 0.9s blank, repeated.
FRAME_LABELS = (["ON"] * 9 + ["OFF"] * 9) * 2


def read_rgb565be(path: Path) -> list[int]:
    data = path.read_bytes()
    if len(data) != IMAGE_BYTES:
        raise ValueError(f"{path} has {len(data)} bytes; expected {IMAGE_BYTES}")

    return [(data[i] << 8) | data[i + 1] for i in range(0, len(data), 2)]


def encode_rgb565be(pixels: list[int]) -> bytes:
    out = bytearray()
    for pixel in pixels:
        out.append((pixel >> 8) & 0xFF)
        out.append(pixel & 0xFF)
    return bytes(out)


def rgb565_to_rgb(pixel: int) -> tuple[int, int, int]:
    r5 = (pixel >> 11) & 0x1F
    g6 = (pixel >> 5) & 0x3F
    b5 = pixel & 0x1F
    return (
        r5 * 255 // 31,
        g6 * 255 // 63,
        b5 * 255 // 31,
    )


def image_to_rgb565be(image: Image.Image) -> bytes:
    image = image.convert("RGB")
    out = bytearray()
    data = image.tobytes()
    for index in range(0, len(data), 3):
        r, g, b = data[index], data[index + 1], data[index + 2]
        pixel = (((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3))
        out.append((pixel >> 8) & 0xFF)
        out.append(pixel & 0xFF)
    return bytes(out)


def frame_to_image(pixels: list[int]) -> Image.Image:
    image = Image.new("RGB", (WIDTH, HEIGHT))
    image.putdata([rgb565_to_rgb(pixel) for pixel in pixels])
    return image


def write_contact_sheet(frames: list[list[int]]) -> None:
    scale = 2
    label_height = 14
    columns = 6
    rows = 6
    cell_width = WIDTH * scale
    cell_height = HEIGHT * scale + label_height
    sheet = Image.new("RGB", (columns * cell_width, rows * cell_height), (0, 0, 0))
    draw = ImageDraw.Draw(sheet)

    for index, frame in enumerate(frames):
        x = (index % columns) * cell_width
        y = (index // columns) * cell_height
        image = frame_to_image(frame).resize((cell_width, HEIGHT * scale), Image.Resampling.NEAREST)
        sheet.paste(image, (x, y + label_height))
        draw.text((x + 4, y + 1), f"A{index:02d}  {FRAME_LABELS[index]}", fill=(90, 250, 131))

    MOCKUP_DIR.mkdir(parents=True, exist_ok=True)
    sheet.save(CONTACT_SHEET)


def build_waiting_screen(blank_pixels: list[int]) -> Image.Image:
    image = frame_to_image(blank_pixels)
    draw = ImageDraw.Draw(image)
    font = ImageFont.truetype(str(FONT_PATH), 29)
    text = "等待写入"
    bbox = draw.textbbox((0, 0), text, font=font)
    text_width = bbox[2] - bbox[0]
    text_height = bbox[3] - bbox[1]
    x = (WIDTH - text_width) // 2 - bbox[0]
    y = (HEIGHT - text_height) // 2 - bbox[1] - 1
    draw.text((x, y), text, font=font, fill=(90, 250, 131))
    return image


def main() -> None:
    base_pixels = read_rgb565be(BASE_ASSET)
    blank_pixels = read_rgb565be(BLANK_ASSET)
    frames = [
        base_pixels if label == "ON" else blank_pixels
        for label in FRAME_LABELS
    ]

    ANIMATION_ASSET.write_bytes(b"".join(encode_rgb565be(frame) for frame in frames))
    write_contact_sheet(frames)

    waiting_screen = build_waiting_screen(blank_pixels)
    WAITING_ASSET.write_bytes(image_to_rgb565be(waiting_screen))
    waiting_screen.save(WAITING_MOCKUP)

    print(f"wrote {ANIMATION_ASSET} ({ANIMATION_ASSET.stat().st_size} bytes)")
    print(f"wrote {CONTACT_SHEET}")
    print(f"wrote {WAITING_ASSET} ({WAITING_ASSET.stat().st_size} bytes)")
    print(f"wrote {WAITING_MOCKUP}")


if __name__ == "__main__":
    main()
