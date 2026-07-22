from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parents[2]
ASSET_DIR = ROOT / "flasher" / "src-tauri" / "assets"
MOCKUP_DIR = ROOT / "docs" / "mockups"

WIDTH = 160
HEIGHT = 80
LOGO_HEIGHT = 68
PAGE_BYTES = 256
RGB_IMAGE_BYTES = WIDTH * HEIGHT * 2
LOGO_PAGE_BYTES = PAGE_BYTES * 6

RESOURCE_DIRECTORY_TEMPLATE = bytes.fromhex(
    "00 00 00 00 00 64 00 24 00 00 00 00 FF FF FF FF FF FF FF FF 00 A0 00 50 FF FF FF FF FF FF FF FF"
    "00 00 02 00 00 64 00 01 00 0E F2 00 FF FF FF FF FF FF FF FF 00 A0 00 50 FF FF FF FF FF FF FF FF"
    "80 01 00 00 00 64 00 01 00 0E EC 00 FF FF FF FF FF FF 00 00 00 A0 00 44 FF FF FF FF FF FF FF FF"
    "80 00 00 00 00 64 FF FF 00 00 00 00 FF FF FF FF FF FF FF FF 00 A0 00 50 FF FF FF FF FF FF FF FF"
)


def patch_u16be(data: bytearray, offset: int, value: int) -> None:
    data[offset] = (value >> 8) & 0xFF
    data[offset + 1] = value & 0xFF


def patch_u24be(data: bytearray, offset: int, value: int) -> None:
    data[offset] = (value >> 16) & 0xFF
    data[offset + 1] = (value >> 8) & 0xFF
    data[offset + 2] = value & 0xFF


def build_resource_directory() -> bytes:
    directory = bytearray(RESOURCE_DIRECTORY_TEMPLATE)
    patch_u16be(directory, 0x04, 900)
    patch_u16be(directory, 0x06, 2)
    patch_u24be(directory, 0x08, 0)
    patch_u16be(directory, 0x20 + 0x04, 100)
    patch_u16be(directory, 0x20 + 0x06, 1)
    patch_u24be(directory, 0x20 + 0x08, 200)
    return bytes(directory).ljust(PAGE_BYTES, b"\xFF")


def encode_mono_msb_left(mask: Image.Image) -> bytes:
    pixels = mask.convert("1")
    out = bytearray()
    for y in range(LOGO_HEIGHT):
        for byte_x in range(20):
            value = 0
            for bit in range(8):
                x = byte_x * 8 + bit
                if pixels.getpixel((x, y)) == 0:
                    value |= 1 << (7 - bit)
            out.append(value)
    return bytes(out).ljust(LOGO_PAGE_BYTES, b"\x00")


def font(path: str, size: int) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(path, size)


def centered_text_x(draw: ImageDraw.ImageDraw, text: str, face: ImageFont.FreeTypeFont) -> int:
    box = draw.textbbox((0, 0), text, font=face)
    return (WIDTH * 4 - (box[2] - box[0])) // 2


def build_logo_mask() -> Image.Image:
    scale = 4
    image = Image.new("L", (WIDTH * scale, LOGO_HEIGHT * scale), 255)
    draw = ImageDraw.Draw(image)
    title_font = font("C:/Windows/Fonts/arialbd.ttf", 28 * scale)
    sub_font = font("C:/Windows/Fonts/arialbd.ttf", 9 * scale)

    draw.rounded_rectangle(
        (6 * scale, 8 * scale, 154 * scale, 60 * scale),
        radius=5 * scale,
        outline=0,
        width=2 * scale,
    )
    draw.line(
        (
            18 * scale,
            18 * scale,
            40 * scale,
            18 * scale,
            50 * scale,
            28 * scale,
            142 * scale,
            28 * scale,
        ),
        fill=0,
        width=2 * scale,
    )
    for x, y in [(14, 14), (46, 24), (136, 24)]:
        draw.ellipse((x * scale, y * scale, (x + 8) * scale, (y + 8) * scale), fill=0)

    title = "MSU2"
    draw.text((centered_text_x(draw, title, title_font), 21 * scale), title, font=title_font, fill=0)

    sub = "IP DISPLAY"
    draw.text((centered_text_x(draw, sub, sub_font), 48 * scale), sub, font=sub_font, fill=0)

    return image.resize((WIDTH, LOGO_HEIGHT), Image.Resampling.LANCZOS).point(
        lambda p: 0 if p < 160 else 255
    )


def rgb565_to_rgb(pixel: int) -> tuple[int, int, int]:
    r5 = (pixel >> 11) & 0x1F
    g6 = (pixel >> 5) & 0x3F
    b5 = pixel & 0x1F
    return (r5 * 255 // 31, g6 * 255 // 63, b5 * 255 // 31)


def rgb565be_to_image(path: Path) -> Image.Image:
    data = path.read_bytes()
    if len(data) != RGB_IMAGE_BYTES:
        raise ValueError(f"{path} has {len(data)} bytes; expected {RGB_IMAGE_BYTES}")

    pixels = [(data[i] << 8) | data[i + 1] for i in range(0, len(data), 2)]
    image = Image.new("RGB", (WIDTH, HEIGHT))
    image.putdata([rgb565_to_rgb(pixel) for pixel in pixels])
    return image


def build_contact_sheet() -> Image.Image:
    scale = 4
    label_height = 18
    visible = rgb565be_to_image(ASSET_DIR / "offline.rgb565be").resize(
        (WIDTH * scale, HEIGHT * scale), Image.Resampling.NEAREST
    )
    blank = rgb565be_to_image(ASSET_DIR / "offline_blank.rgb565be").resize(
        (WIDTH * scale, HEIGHT * scale), Image.Resampling.NEAREST
    )

    sheet = Image.new(
        "RGB", (WIDTH * scale * 2, HEIGHT * scale + label_height), (255, 255, 255)
    )
    draw = ImageDraw.Draw(sheet)
    sheet.paste(visible, (0, label_height))
    sheet.paste(blank, (WIDTH * scale, label_height))
    draw.text((6, 2), "E0 frame 0: visible", fill=(0, 0, 0))
    draw.text((WIDTH * scale + 6, 2), "E0 frame 1: blank", fill=(0, 0, 0))
    return sheet


def main() -> None:
    ASSET_DIR.mkdir(parents=True, exist_ok=True)
    MOCKUP_DIR.mkdir(parents=True, exist_ok=True)

    directory_path = ASSET_DIR / "resource_directory.bin"
    logo_path = ASSET_DIR / "mlogo_160x68.mono"

    directory_path.write_bytes(build_resource_directory())

    logo_mask = build_logo_mask()
    logo_path.write_bytes(encode_mono_msb_left(logo_mask))

    preview = Image.new("RGB", (WIDTH, LOGO_HEIGHT), (255, 255, 255))
    blue = Image.new("RGB", (WIDTH, LOGO_HEIGHT), (0, 0, 255))
    preview.paste(blue, mask=logo_mask.point(lambda p: 255 if p == 0 else 0))
    preview.resize((WIDTH * 4, LOGO_HEIGHT * 4), Image.Resampling.NEAREST).save(
        MOCKUP_DIR / "msu2-boot-logo-preview.png"
    )

    build_contact_sheet().save(MOCKUP_DIR / "msu2-offline-animation-contact-sheet.png")

    print(f"flasher/src-tauri/assets/mlogo_160x68.mono has {logo_path.stat().st_size} bytes")
    print(
        f"flasher/src-tauri/assets/resource_directory.bin has {directory_path.stat().st_size} bytes"
    )


if __name__ == "__main__":
    main()
