# 紧凑 Flash 布局和启动 Logo 实现计划

> **给执行代理的要求：** 实施本计划时必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，按任务逐项执行。步骤使用 checkbox（`- [ ]`）格式跟踪。

**目标：** 把现在浪费空间的 36 帧离线动图改成 2 帧紧凑布局，把 host 端状态图移出字库、启动 logo、离线静图槽位，恢复离线静图，并加入新的 160x68 单色启动 logo。

**架构：** 不再把外部 Flash 当成一组固定 100 页彩色图片，而是建模成“不同类型、不同页数”的资源列表。flasher 负责写入紧凑资源和 page `4094` 的资源目录；host-usb 迁移到新的状态页；官方数码管字形仍保留在 `4026+`。

**技术栈：** Rust/Tauri flasher 后端、现有 MSU2 串口协议工具、Python/Pillow 资源生成脚本、Rust host-usb 程序、RGB565BE 彩色图格式、1bpp 单色图格式。

## 全局约束

- 不修改 MCU 固件，也不改 USB/串口协议。
- 串口继续使用 `921600 8N1` 和 RTS/CTS 硬件流控。
- 官方数码管字形资源继续保留在 `4026..4037`，host 显示 IP 时仍使用 `4026 + digit`。
- page `4095` 不写入，因为完整厂商镜像可能在这里放屏幕 panel 配置。
- 不再使用当前 `3726..3825` 的 DHCP 失败图范围，因为它会覆盖启动 logo 的 `3820..3825`。
- 不再把 `3826..3925` 用作 host 专用的“获取 IP 中”状态，因为资源目录 E1 当前把它当作离线静图槽位。
- 全屏彩色图固定为 `160x80 RGB565BE`，大小 `25_600` bytes，占 `100` 页。
- 启动 logo 固定为 `160x68 1bpp`，按行存储，每行 20 bytes，MSB 在左，补齐到 `1536` bytes，占 `6` 页。
- 资源目录是 page `4094` 的一个 256-byte 页。实现时在 PC 端用官方模板生成完整 256-byte `resource_directory.bin`，只 patch 已确认字段，未知字段保留官方原始字节；刷写时仍然是擦除 page `4094` 并整页写回，不做设备端局部 byte patch。
- 目标布局假设固件会遵守资源目录里的 `count`。这个判断来自低速厂商镜像已经证明 `interval` 生效；实现后再通过人工刷机验收确认 `count=2`。

---

## 目标 Flash 布局

| 用途 | 页范围 | 页数 | 资源来源 |
| --- | ---: | ---: | --- |
| 离线动图可见帧 | `0..99` | 100 | `flasher/src-tauri/assets/offline.rgb565be` |
| 离线动图空白帧 | `100..199` | 100 | `flasher/src-tauri/assets/ip_bg.rgb565be` |
| 离线静图 | `200..299` | 100 | `flasher/src-tauri/assets/offline.rgb565be` |
| Host 获取 IP 中 | `300..399` | 100 | `flasher/src-tauri/assets/acquiring.rgb565be` |
| Host DHCP 失败 | `400..499` | 100 | `flasher/src-tauri/assets/dhcp_failed.rgb565be` |
| Host IP 背景 | `500..599` | 100 | `flasher/src-tauri/assets/ip_bg.rgb565be` |
| 启动 logo | `3820..3825` | 6 | `flasher/src-tauri/assets/mlogo_160x68.mono` |
| 官方数码管字形 | `4026..4037` | 12 | 保留，不由 flasher 写入 |
| 资源目录 | `4094` | 1 | `flasher/src-tauri/assets/resource_directory.bin` |
| panel 配置尾页 | `4095` | 1 | 保留，不由 flasher 写入 |

新布局会释放 `600..3599`，同时停止覆盖 `3651..3778` ASCII 字库、`3820..3825` 启动 logo 和 `3826..3925` 离线静图槽位。

## 资源目录写法

以官方京东方 MINI 资源目录作为模板。flags 和未知字节保留原始值。这个模板不是从设备实时读出的，而是来自仓库里的厂商参考镜像；生成脚本会先在 PC 端构造完整 page `4094` 内容，再交给 flasher 按普通 Flash 页写入流程擦写：

```text
0x00 E0 template: 00 00 00 00 00 64 00 24 00 00 00 00 FF FF FF FF FF FF FF FF 00 A0 00 50 FF FF FF FF FF FF FF FF
0x20 E1 template: 00 00 02 00 00 64 00 01 00 0E F2 00 FF FF FF FF FF FF FF FF 00 A0 00 50 FF FF FF FF FF FF FF FF
0x40 E2 template: 80 01 00 00 00 64 00 01 00 0E EC 00 FF FF FF FF FF FF 00 00 00 A0 00 44 FF FF FF FF FF FF FF FF
0x60 E3 template: 80 00 00 00 00 64 FF FF 00 00 00 00 FF FF FF FF FF FF FF FF 00 A0 00 50 FF FF FF FF FF FF FF FF
```

只改下面这些字段：

| Entry | 字段 | 新值 | 字节 |
| ---: | --- | ---: | --- |
| E0 | interval | `900ms` | `03 84`，位于 `+0x04..+0x05` |
| E0 | count | `2` 帧 | `00 02`，位于 `+0x06..+0x07` |
| E0 | start page | `0` | `00 00 00`，位于 `+0x08..+0x0A` |
| E1 | interval | 保持 `100ms` | `00 64` |
| E1 | count | `1` 帧 | `00 01` |
| E1 | start page | `200` | `00 00 C8`，位于 `+0x28..+0x2A` |
| E2 | 不变 | page `3820`，尺寸 `160x68` | 保留官方字节 |
| E3 | 不变 | 占位/结束 entry | 保留官方字节 |

注意：SPI Flash 不能安全地只把一个页里的几个字节“原地改掉”。正常流程是准备完整 256-byte 页内容，擦除目标页，再写回完整页。MSU2 协议里虽然整理出了读 Flash 字节命令，但当前计划不依赖读回设备上的 page `4094`；这样实现更简单，也和我们现在基于官方 MINI 镜像重打资源包的做法一致。

---

## 文件结构

- 修改 `flasher/src-tauri/src/assets.rs`
  - 把固定 100 页图片的 `FlashImage` 假设改成可变页数的 `FlashAsset`。
  - 定义紧凑布局常量，供 flasher 测试和实现使用。
  - 嵌入 `mlogo_160x68.mono` 和 `resource_directory.bin`。
  - 把布局校验改成“受保护页范围”校验。
- 修改 `flasher/src-tauri/src/flasher.rs`
  - 擦除和写入时使用每个资源自己的 `page_count`。
  - 进度总页数改成所有资源页数之和。
  - 更新 debug preview 使用的新页号。
- 新建 `flasher/tools/generate_flash_assets.py`
  - 生成新启动 logo 单色资源和预览 PNG。
  - 生成 `resource_directory.bin`。
  - 生成紧凑两帧离线动图对照预览图。
- 删除 `flasher/src-tauri/assets/offline_animation.rgb565be`
  - 代码不再嵌入这个 36 帧大资源后删除，避免以后误用。
- 新建 `flasher/src-tauri/assets/mlogo_160x68.mono`
- 新建 `flasher/src-tauri/assets/resource_directory.bin`
- 修改 `host-usb/src/protocol.rs`
  - host 状态页迁移到 `300`、`400`、`500`。
- 修改 `host-usb/src/display.rs`
  - 更新新页号对应的 packet 测试。
- 修改 `docs/msu2-protocol-and-flash-layout.md`
  - 加入当前项目紧凑布局。
- 修改 `docs/flasher-notes.md`
  - 记录 flasher 实际刷写的资源列表和保留区。
- 修改 `docs/host-usb-ip-display-draft.md`
  - 记录 host-usb v1 使用的新页号。

---

## 任务 1：加入紧凑布局模型和测试

**文件：**
- 修改：`flasher/src-tauri/src/assets.rs`

**接口：**
- 产出：
  - `pub const PAGE_BYTES: usize = 256`
  - `pub const RGB_IMAGE_BYTES: usize = 25_600`
  - `pub const RGB_IMAGE_PAGES: u16 = 100`
  - `pub const MONO_LOGO_PAGES: u16 = 6`
  - `pub const DIRECTORY_PAGES: u16 = 1`
  - `pub const OFFLINE_VISIBLE_PAGE: u16 = 0`
  - `pub const OFFLINE_BLANK_PAGE: u16 = 100`
  - `pub const OFFLINE_STATIC_PAGE: u16 = 200`
  - `pub const HOST_PENDING_PAGE: u16 = 300`
  - `pub const HOST_DHCP_FAILED_PAGE: u16 = 400`
  - `pub const HOST_IP_BG_PAGE: u16 = 500`
  - `pub const STARTUP_LOGO_PAGE: u16 = 3820`
  - `pub const DIGIT_RESOURCE_PAGE: u16 = 4026`
  - `pub const RESOURCE_DIRECTORY_PAGE: u16 = 4094`
  - `pub const PANEL_CONFIG_PAGE: u16 = 4095`
  - `pub struct FlashAsset<'a> { pub label: &'static str, pub start_page: u16, pub page_count: u16, pub bytes: &'a [u8] }`
  - `impl FlashAsset<'_> { pub fn expected_len(&self) -> usize; pub fn end_page(&self) -> u16; }`
  - `pub fn validate_asset(asset: &FlashAsset<'_>) -> Result<(), AssetError>`
- 消费：
  - 现有 `AssetError::WrongSize`
  - 当前仍引用 `FlashImage` 的 flasher 代码

- [ ] **步骤 1：写失败测试**

把下面测试加入 `flasher/src-tauri/src/assets.rs` 的 `#[cfg(test)] mod tests`：

```rust
#[test]
fn compact_layout_constants_are_stable() {
    assert_eq!(OFFLINE_VISIBLE_PAGE, 0);
    assert_eq!(OFFLINE_BLANK_PAGE, 100);
    assert_eq!(OFFLINE_STATIC_PAGE, 200);
    assert_eq!(HOST_PENDING_PAGE, 300);
    assert_eq!(HOST_DHCP_FAILED_PAGE, 400);
    assert_eq!(HOST_IP_BG_PAGE, 500);
    assert_eq!(STARTUP_LOGO_PAGE, 3820);
    assert_eq!(DIGIT_RESOURCE_PAGE, 4026);
    assert_eq!(RESOURCE_DIRECTORY_PAGE, 4094);
    assert_eq!(PANEL_CONFIG_PAGE, 4095);
}

#[test]
fn flash_asset_end_page_uses_page_count() {
    let bytes = [0u8; PAGE_BYTES * 6];
    let asset = FlashAsset {
        label: "logo",
        start_page: STARTUP_LOGO_PAGE,
        page_count: MONO_LOGO_PAGES,
        bytes: &bytes,
    };

    assert_eq!(asset.expected_len(), PAGE_BYTES * 6);
    assert_eq!(asset.end_page(), 3825);
}

#[test]
fn validates_variable_sized_assets() {
    let rgb = [0u8; RGB_IMAGE_BYTES];
    let logo = [0u8; PAGE_BYTES * 6];
    let directory = [0u8; PAGE_BYTES];

    validate_asset(&FlashAsset {
        label: "rgb",
        start_page: HOST_PENDING_PAGE,
        page_count: RGB_IMAGE_PAGES,
        bytes: &rgb,
    })
    .unwrap();
    validate_asset(&FlashAsset {
        label: "logo",
        start_page: STARTUP_LOGO_PAGE,
        page_count: MONO_LOGO_PAGES,
        bytes: &logo,
    })
    .unwrap();
    validate_asset(&FlashAsset {
        label: "directory",
        start_page: RESOURCE_DIRECTORY_PAGE,
        page_count: DIRECTORY_PAGES,
        bytes: &directory,
    })
    .unwrap();
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test --manifest-path flasher/src-tauri/Cargo.toml assets::tests::compact_layout_constants_are_stable assets::tests::flash_asset_end_page_uses_page_count assets::tests::validates_variable_sized_assets
```

期望：失败，因为新常量和 `FlashAsset` 还不存在。

- [ ] **步骤 3：实现资源模型**

把固定 `FlashImage` 类型替换成下面结构：

```rust
pub const PAGE_BYTES: usize = 256;
pub const RGB_IMAGE_BYTES: usize = 25_600;
pub const IMAGE_BYTES: usize = RGB_IMAGE_BYTES;
pub const RGB_IMAGE_PAGES: u16 = 100;
pub const PAGES_PER_IMAGE: u16 = RGB_IMAGE_PAGES;
pub const MONO_LOGO_PAGES: u16 = 6;
pub const DIRECTORY_PAGES: u16 = 1;

pub const OFFLINE_VISIBLE_PAGE: u16 = 0;
pub const OFFLINE_BLANK_PAGE: u16 = 100;
pub const OFFLINE_STATIC_PAGE: u16 = 200;
pub const HOST_PENDING_PAGE: u16 = 300;
pub const HOST_DHCP_FAILED_PAGE: u16 = 400;
pub const HOST_IP_BG_PAGE: u16 = 500;
pub const STARTUP_LOGO_PAGE: u16 = 3820;
pub const DIGIT_RESOURCE_PAGE: u16 = 4026;
pub const RESOURCE_DIRECTORY_PAGE: u16 = 4094;
pub const PANEL_CONFIG_PAGE: u16 = 4095;

#[derive(Debug, Clone, Copy)]
pub struct FlashAsset<'a> {
    pub label: &'static str,
    pub start_page: u16,
    pub page_count: u16,
    pub bytes: &'a [u8],
}

impl<'a> FlashAsset<'a> {
    pub fn expected_len(&self) -> usize {
        self.page_count as usize * PAGE_BYTES
    }

    pub fn end_page(&self) -> u16 {
        self.start_page + self.page_count - 1
    }
}

pub type FlashImage<'a> = FlashAsset<'a>;
```

本任务里先保留 `IMAGE_BYTES`、`PAGES_PER_IMAGE` 和 `FlashImage` 别名，让旧测试在任务 4 之前还能编译。

- [ ] **步骤 4：增加校验入口**

新增：

```rust
pub fn validate_asset(asset: &FlashAsset<'_>) -> Result<(), AssetError> {
    let expected = asset.expected_len();
    if asset.bytes.len() != expected {
        return Err(AssetError::WrongSize {
            label: asset.label,
            actual: asset.bytes.len(),
            expected,
        });
    }

    Ok(())
}
```

保留旧的 `validate_image(label, bytes)`，让它包装新实现：

```rust
pub fn validate_image(label: &'static str, bytes: &[u8]) -> Result<(), AssetError> {
    validate_asset(&FlashAsset {
        label,
        start_page: 0,
        page_count: RGB_IMAGE_PAGES,
        bytes,
    })
}
```

- [ ] **步骤 5：运行局部测试**

运行：

```powershell
cargo test --manifest-path flasher/src-tauri/Cargo.toml assets::tests -- --nocapture
```

期望：新资源模型测试通过；旧固定刷写计划测试可能要到任务 3 才全部修完。

- [ ] **步骤 6：提交**

```powershell
git add flasher/src-tauri/src/assets.rs
git commit -m "refactor: model variable flash assets"
```

## 任务 2：生成资源目录和启动 logo 资产

**文件：**
- 新建：`flasher/tools/generate_flash_assets.py`
- 新建：`flasher/src-tauri/assets/mlogo_160x68.mono`
- 新建：`flasher/src-tauri/assets/resource_directory.bin`
- 修改：`docs/mockups/msu2-boot-logo-preview.png`
- 修改：`docs/mockups/msu2-offline-animation-contact-sheet.png`

**接口：**
- 产出：
  - `resource_directory.bin` 必须正好 `256` bytes。
  - `mlogo_160x68.mono` 必须正好 `1536` bytes。
  - preview PNG 使用白底蓝字，匹配固件启动 logo 的显示颜色。
- 消费：
  - `flasher/src-tauri/assets/offline.rgb565be`
  - `flasher/src-tauri/assets/ip_bg.rgb565be`
  - Windows 上的 `C:/Windows/Fonts/arialbd.ttf`。启动 logo 以 Latin/pictorial 为主，因为 1bpp 单色尺寸很小。

- [ ] **步骤 1：创建生成脚本**

创建 `flasher/tools/generate_flash_assets.py`，核心内容如下：

```python
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
```

logo 编码和绘制逻辑：

```python
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


def build_logo_mask() -> Image.Image:
    scale = 4
    image = Image.new("L", (WIDTH * scale, LOGO_HEIGHT * scale), 255)
    draw = ImageDraw.Draw(image)
    title_font = ImageFont.truetype("C:/Windows/Fonts/arialbd.ttf", 28 * scale)
    sub_font = ImageFont.truetype("C:/Windows/Fonts/arialbd.ttf", 9 * scale)
    draw.rounded_rectangle((6 * scale, 8 * scale, 154 * scale, 60 * scale), radius=5 * scale, outline=0, width=2 * scale)
    draw.line((18 * scale, 18 * scale, 40 * scale, 18 * scale, 50 * scale, 28 * scale, 142 * scale, 28 * scale), fill=0, width=2 * scale)
    draw.ellipse((14 * scale, 14 * scale, 22 * scale, 22 * scale), fill=0)
    draw.ellipse((46 * scale, 24 * scale, 54 * scale, 32 * scale), fill=0)
    draw.ellipse((136 * scale, 24 * scale, 144 * scale, 32 * scale), fill=0)
    title = "MSU2"
    title_box = draw.textbbox((0, 0), title, font=title_font)
    title_x = (WIDTH * scale - (title_box[2] - title_box[0])) // 2
    draw.text((title_x, 21 * scale), title, font=title_font, fill=0)
    sub = "IP DISPLAY"
    sub_box = draw.textbbox((0, 0), sub, font=sub_font)
    sub_x = (WIDTH * scale - (sub_box[2] - sub_box[0])) // 2
    draw.text((sub_x, 48 * scale), sub, font=sub_font, fill=0)
    return image.resize((WIDTH, LOGO_HEIGHT), Image.Resampling.LANCZOS).point(lambda p: 0 if p < 160 else 255)
```

对照预览图和文件写入逻辑：

```python
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
    visible = rgb565be_to_image(ASSET_DIR / "offline.rgb565be").resize((WIDTH * scale, HEIGHT * scale), Image.Resampling.NEAREST)
    blank = rgb565be_to_image(ASSET_DIR / "ip_bg.rgb565be").resize((WIDTH * scale, HEIGHT * scale), Image.Resampling.NEAREST)
    sheet = Image.new("RGB", (WIDTH * scale * 2, HEIGHT * scale + label_height), (255, 255, 255))
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
    preview.resize((WIDTH * 4, LOGO_HEIGHT * 4), Image.Resampling.NEAREST).save(MOCKUP_DIR / "msu2-boot-logo-preview.png")

    build_contact_sheet().save(MOCKUP_DIR / "msu2-offline-animation-contact-sheet.png")

    print(f"flasher/src-tauri/assets/mlogo_160x68.mono has {logo_path.stat().st_size} bytes")
    print(f"flasher/src-tauri/assets/resource_directory.bin has {directory_path.stat().st_size} bytes")


if __name__ == "__main__":
    main()
```

- [ ] **步骤 2：运行生成脚本**

运行：

```powershell
python flasher/tools/generate_flash_assets.py
```

期望输出：

```text
flasher/src-tauri/assets/mlogo_160x68.mono has 1536 bytes
flasher/src-tauri/assets/resource_directory.bin has 256 bytes
```

- [ ] **步骤 3：验证文件大小**

运行：

```powershell
(Get-Item flasher\src-tauri\assets\mlogo_160x68.mono).Length
(Get-Item flasher\src-tauri\assets\resource_directory.bin).Length
```

期望：

```text
1536
256
```

- [ ] **步骤 4：提交**

```powershell
git add flasher/tools/generate_flash_assets.py flasher/src-tauri/assets/mlogo_160x68.mono flasher/src-tauri/assets/resource_directory.bin docs/mockups/msu2-boot-logo-preview.png docs/mockups/msu2-offline-animation-contact-sheet.png
git commit -m "feat: generate compact flash metadata assets"
```

## 任务 3：生成紧凑刷写计划

**文件：**
- 修改：`flasher/src-tauri/src/assets.rs`
- 删除：`flasher/src-tauri/assets/offline_animation.rgb565be`

**接口：**
- 产出：
  - `pub struct EmbeddedAssets { offline: &'static [u8], acquiring: &'static [u8], dhcp_failed: &'static [u8], ip_bg: &'static [u8], startup_logo: &'static [u8], resource_directory: &'static [u8] }`
  - `pub fn fixed_flash_plan<'a>(assets: &'a EmbeddedAssets) -> Vec<FlashAsset<'a>>`
  - `pub fn validate_plan(plan: &[FlashAsset<'_>]) -> Result<(), AssetError>`
- 消费：
  - 任务 2 生成的 `resource_directory.bin`
  - 任务 2 生成的 `mlogo_160x68.mono`

- [ ] **步骤 1：写失败测试**

把旧的 39 张图测试替换成：

```rust
#[test]
fn embedded_assets_have_verified_size() {
    let assets = embedded_assets();
    assert_eq!(assets.offline.len(), RGB_IMAGE_BYTES);
    assert_eq!(assets.acquiring.len(), RGB_IMAGE_BYTES);
    assert_eq!(assets.dhcp_failed.len(), RGB_IMAGE_BYTES);
    assert_eq!(assets.ip_bg.len(), RGB_IMAGE_BYTES);
    assert_eq!(assets.startup_logo.len(), PAGE_BYTES * MONO_LOGO_PAGES as usize);
    assert_eq!(assets.resource_directory.len(), PAGE_BYTES);
}

#[test]
fn compact_plan_writes_expected_assets_in_order() {
    let assets = embedded_assets();
    let plan = fixed_flash_plan(&assets);
    let labels: Vec<&'static str> = plan.iter().map(|asset| asset.label).collect();
    assert_eq!(
        labels,
        vec![
            "offline_visible",
            "offline_blank",
            "offline_static",
            "pending",
            "dhcp_failed",
            "ip_bg",
            "startup_logo",
            "resource_directory",
        ]
    );
    assert_eq!(plan[0].start_page, OFFLINE_VISIBLE_PAGE);
    assert_eq!(plan[1].start_page, OFFLINE_BLANK_PAGE);
    assert_eq!(plan[2].start_page, OFFLINE_STATIC_PAGE);
    assert_eq!(plan[3].start_page, HOST_PENDING_PAGE);
    assert_eq!(plan[4].start_page, HOST_DHCP_FAILED_PAGE);
    assert_eq!(plan[5].start_page, HOST_IP_BG_PAGE);
    assert_eq!(plan[6].start_page, STARTUP_LOGO_PAGE);
    assert_eq!(plan[7].start_page, RESOURCE_DIRECTORY_PAGE);
}

#[test]
fn compact_plan_preserves_official_font_digit_and_panel_pages() {
    let assets = embedded_assets();
    let plan = fixed_flash_plan(&assets);
    validate_plan(&plan).unwrap();

    for asset in &plan {
        assert!(!(asset.start_page <= 3778 && asset.end_page() >= 3651), "{:?}", asset);
        assert!(!(asset.start_page <= 4037 && asset.end_page() >= DIGIT_RESOURCE_PAGE), "{:?}", asset);
        assert!(asset.end_page() < PANEL_CONFIG_PAGE, "{:?}", asset);
    }
}

#[test]
fn resource_directory_points_offline_animation_to_two_frames() {
    let assets = embedded_assets();
    let bytes = assets.resource_directory;
    assert_eq!(&bytes[0x04..0x08], &[0x03, 0x84, 0x00, 0x02]);
    assert_eq!(&bytes[0x08..0x0b], &[0x00, 0x00, 0x00]);
    assert_eq!(&bytes[0x20 + 0x04..0x20 + 0x08], &[0x00, 0x64, 0x00, 0x01]);
    assert_eq!(&bytes[0x20 + 0x08..0x20 + 0x0b], &[0x00, 0x00, 0xC8]);
    assert_eq!(&bytes[0x40 + 0x08..0x40 + 0x0b], &[0x00, 0x0E, 0xEC]);
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test --manifest-path flasher/src-tauri/Cargo.toml assets::tests -- --nocapture
```

期望：失败，因为 `EmbeddedAssets` 仍在嵌入 `offline_animation`，`fixed_flash_plan` 仍输出 39 个固定大小资源。

- [ ] **步骤 3：更新嵌入资源**

把 `EmbeddedAssets` 和 `embedded_assets()` 改成：

```rust
#[derive(Debug, Clone, Copy)]
pub struct EmbeddedAssets {
    pub offline: &'static [u8],
    pub acquiring: &'static [u8],
    pub dhcp_failed: &'static [u8],
    pub ip_bg: &'static [u8],
    pub startup_logo: &'static [u8],
    pub resource_directory: &'static [u8],
}

pub fn embedded_assets() -> EmbeddedAssets {
    EmbeddedAssets {
        offline: include_bytes!("../assets/offline.rgb565be"),
        acquiring: include_bytes!("../assets/acquiring.rgb565be"),
        dhcp_failed: include_bytes!("../assets/dhcp_failed.rgb565be"),
        ip_bg: include_bytes!("../assets/ip_bg.rgb565be"),
        startup_logo: include_bytes!("../assets/mlogo_160x68.mono"),
        resource_directory: include_bytes!("../assets/resource_directory.bin"),
    }
}
```

- [ ] **步骤 4：更新固定刷写计划**

实现：

```rust
pub fn fixed_flash_plan<'a>(assets: &'a EmbeddedAssets) -> Vec<FlashAsset<'a>> {
    vec![
        FlashAsset {
            label: "offline_visible",
            start_page: OFFLINE_VISIBLE_PAGE,
            page_count: RGB_IMAGE_PAGES,
            bytes: assets.offline,
        },
        FlashAsset {
            label: "offline_blank",
            start_page: OFFLINE_BLANK_PAGE,
            page_count: RGB_IMAGE_PAGES,
            bytes: assets.ip_bg,
        },
        FlashAsset {
            label: "offline_static",
            start_page: OFFLINE_STATIC_PAGE,
            page_count: RGB_IMAGE_PAGES,
            bytes: assets.offline,
        },
        FlashAsset {
            label: "pending",
            start_page: HOST_PENDING_PAGE,
            page_count: RGB_IMAGE_PAGES,
            bytes: assets.acquiring,
        },
        FlashAsset {
            label: "dhcp_failed",
            start_page: HOST_DHCP_FAILED_PAGE,
            page_count: RGB_IMAGE_PAGES,
            bytes: assets.dhcp_failed,
        },
        FlashAsset {
            label: "ip_bg",
            start_page: HOST_IP_BG_PAGE,
            page_count: RGB_IMAGE_PAGES,
            bytes: assets.ip_bg,
        },
        FlashAsset {
            label: "startup_logo",
            start_page: STARTUP_LOGO_PAGE,
            page_count: MONO_LOGO_PAGES,
            bytes: assets.startup_logo,
        },
        FlashAsset {
            label: "resource_directory",
            start_page: RESOURCE_DIRECTORY_PAGE,
            page_count: DIRECTORY_PAGES,
            bytes: assets.resource_directory,
        },
    ]
}
```

- [ ] **步骤 5：更新布局校验**

使用显式受保护范围：

```rust
const PROTECTED_RANGES: &[(u16, u16, &str)] = &[
    (3651, 3778, "ASC64 font"),
    (DIGIT_RESOURCE_PAGE, 4037, "digit glyphs"),
    (4038, 4044, "MP1 mono image"),
    (PANEL_CONFIG_PAGE, PANEL_CONFIG_PAGE, "panel config"),
];
```

允许 `STARTUP_LOGO_PAGE..3825` 和 `RESOURCE_DIRECTORY_PAGE`，因为它们是本计划明确要写的资源。其他资源如果撞到受保护区，测试必须失败。

- [ ] **步骤 6：删除旧 36 帧资源**

运行：

```powershell
git rm flasher\src-tauri\assets\offline_animation.rgb565be
```

- [ ] **步骤 7：运行局部测试**

运行：

```powershell
cargo test --manifest-path flasher/src-tauri/Cargo.toml assets::tests -- --nocapture
```

期望：全部 `assets` 测试通过。

- [ ] **步骤 8：提交**

```powershell
git add flasher/src-tauri/src/assets.rs
git commit -m "feat: use compact flash asset plan"
```

## 任务 4：刷写逻辑改为按资源页数写入

**文件：**
- 修改：`flasher/src-tauri/src/flasher.rs`

**接口：**
- 消费：
  - `FlashAsset.page_count`
  - `validate_asset`
- 产出：
  - `flash_images` 和 `flash_images_with_screen_status` 接收 `&[FlashAsset<'_>]`。
  - 进度总数改为 `sum(asset.page_count)`。

- [ ] **步骤 1：写失败测试**

更新 `flasher/src-tauri/src/flasher.rs` 测试：

```rust
#[test]
fn writes_variable_sized_assets() {
    let logo = vec![0x5a; 256 * 6];
    let directory = vec![0xa5; 256];
    let assets = [
        FlashAsset {
            label: "logo",
            start_page: 3820,
            page_count: 6,
            bytes: &logo,
        },
        FlashAsset {
            label: "directory",
            start_page: 4094,
            page_count: 1,
            bytes: &directory,
        },
    ];
    let mut port = MockPort::new();
    let mut progress = Vec::new();

    flash_images(&mut port, &assets, |event| progress.push(event)).unwrap();

    assert_eq!(port.writes.len(), 9);
    assert_eq!(port.writes[0], vec![0x03, 0x02, 0x0e, 0xec, 0x00, 0x06]);
    assert_eq!(&port.writes[1][384..390], &[0x03, 0x03, 0x00, 0x0e, 0xec, 0x01]);
    assert_eq!(&port.writes[6][384..390], &[0x03, 0x03, 0x00, 0x0e, 0xf1, 0x01]);
    assert_eq!(port.writes[7], vec![0x03, 0x02, 0x0f, 0xfe, 0x00, 0x01]);
    assert_eq!(&port.writes[8][384..390], &[0x03, 0x03, 0x00, 0x0f, 0xfe, 0x01]);
    assert_eq!(progress.last().unwrap().percent, 100);
    assert_eq!(progress.last().unwrap().total_pages, 7);
}
```

现有测试里构造 `FlashImage` 的地方都加上 `page_count: RGB_IMAGE_PAGES`。

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test --manifest-path flasher/src-tauri/Cargo.toml flasher::tests::writes_variable_sized_assets -- --nocapture
```

期望：失败，因为 `flash_images_internal` 仍按 `PAGES_PER_IMAGE` 擦写。

- [ ] **步骤 3：替换固定页数循环**

在 `flash_images_internal` 中把校验和总数计算改成：

```rust
for image in images {
    validate_asset(image).map_err(|err| AppError::Asset(err.to_string()))?;
}

let total_pages: u32 = images.iter().map(|image| image.page_count as u32).sum();
```

把擦除和写入循环改成：

```rust
erase_pages(
    port,
    image.start_page,
    image.page_count,
    RetryPolicy::default(),
)?;

for page_index in 0..image.page_count {
    let offset = (page_index as usize) * PAGE_BYTES;
    let mut chunk = [0u8; PAGE_BYTES];
    chunk.copy_from_slice(&image.bytes[offset..offset + PAGE_BYTES]);
    let page = image.start_page as u32 + page_index as u32;
    write_page(port, page, &chunk, RetryPolicy::default())?;
    completed_pages += 1;
    // 现有 progress emit 逻辑留在这里
}
```

- [ ] **步骤 4：更新导入项**

把：

```rust
use crate::assets::{validate_image, FlashImage, PAGES_PER_IMAGE};
```

改成：

```rust
use crate::assets::{validate_asset, FlashAsset, PAGE_BYTES, RGB_IMAGE_PAGES};
```

如果测试还暂时依赖 alias，可以先保留 `FlashImage`，但最终应优先使用 `FlashAsset` 命名。

- [ ] **步骤 5：更新预览辅助函数页号**

把 `preview_pages` 中的旧页号：

```rust
for page in [0u16, 3826, 3926, 0] {
```

改成：

```rust
for page in [OFFLINE_VISIBLE_PAGE, HOST_PENDING_PAGE, HOST_IP_BG_PAGE, OFFLINE_VISIBLE_PAGE] {
```

从 `assets.rs` import 这 3 个常量。

- [ ] **步骤 6：运行 flasher 测试**

运行：

```powershell
cargo test --manifest-path flasher/src-tauri/Cargo.toml flasher::tests -- --nocapture
```

期望：全部 flasher 测试通过。

- [ ] **步骤 7：提交**

```powershell
git add flasher/src-tauri/src/flasher.rs
git commit -m "refactor: flash variable page assets"
```

## 任务 5：把 host 显示页迁移到紧凑布局

**文件：**
- 修改：`host-usb/src/protocol.rs`
- 修改：`host-usb/src/display.rs`

**接口：**
- 产出：
  - `pub const DHCP_FAILED_PAGE: u16 = 400`
  - `pub const PENDING_PAGE: u16 = 300`
  - `pub const IP_BACKGROUND_PAGE: u16 = 500`
  - `pub const DIGIT_RESOURCE_PAGE: u16 = 4026`
- 消费：
  - 现有 `DisplayRenderer` 行为。

- [ ] **步骤 1：写失败测试**

更新 `host-usb/src/protocol.rs` 测试：

```rust
#[test]
fn compact_layout_page_packets_match_expected_bytes() {
    assert_eq!(show_photo_packet(PENDING_PAGE), [0x02, 0x03, 0x00, 0x01, 0x2c, 0x00]);
    assert_eq!(show_photo_packet(DHCP_FAILED_PAGE), [0x02, 0x03, 0x00, 0x01, 0x90, 0x00]);
    assert_eq!(show_photo_packet(IP_BACKGROUND_PAGE), [0x02, 0x03, 0x00, 0x01, 0xf4, 0x00]);
    assert_eq!(load_ram_mix_show_packet(IP_BACKGROUND_PAGE), [0x02, 0x03, 0x11, 0x01, 0xf4, 0x00]);
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test --manifest-path host-usb/Cargo.toml protocol::tests::compact_layout_page_packets_match_expected_bytes -- --nocapture
```

期望：失败，因为常量仍指向 `3726/3826/3926`。

- [ ] **步骤 3：更新常量**

把 `host-usb/src/protocol.rs` 改成：

```rust
pub const DHCP_FAILED_PAGE: u16 = 400;
pub const PENDING_PAGE: u16 = 300;
pub const IP_BACKGROUND_PAGE: u16 = 500;
pub const DIGIT_RESOURCE_PAGE: u16 = 4026;
```

- [ ] **步骤 4：更新显示测试**

把 `host-usb/src/display.rs` 中 RAM mix packet 的期望改成：

```rust
assert!(writes
    .iter()
    .any(|write| write.bytes == [0x02, 0x03, 0x11, 0x01, 0xf4, 0x00]));
```

数码管字形测试保持不变，因为字形基址还是 `4026`。

- [ ] **步骤 5：运行 host-usb 测试**

运行：

```powershell
cargo test --manifest-path host-usb/Cargo.toml -- --nocapture
```

期望：全部 host-usb 测试通过。

- [ ] **步骤 6：提交**

```powershell
git add host-usb/src/protocol.rs host-usb/src/display.rs
git commit -m "fix: move host display pages to compact layout"
```

## 任务 6：更新 Flash 布局文档

**文件：**
- 修改：`docs/msu2-protocol-and-flash-layout.md`
- 修改：`docs/flasher-notes.md`
- 修改：`docs/host-usb-ip-display-draft.md`

**接口：**
- 产出：
  - 一个和本计划一致的紧凑布局表。
  - 一个资源目录表，明确 E0 `count=2`、`interval=900`，E1 page `200`。
  - 一个 host-usb 说明，记录 v1 使用静态 flashed pages 显示 pending、DHCP failed、IP background。

- [ ] **步骤 1：更新协议和布局文档**

在 `docs/msu2-protocol-and-flash-layout.md` 加入 `Current Project Compact Layout` 章节，表格内容：

```markdown
| 用途 | 页 | 备注 |
| --- | ---: | --- |
| 离线动图可见帧 | `0..99` | E0 frame 0 |
| 离线动图空白帧 | `100..199` | E0 frame 1 |
| 离线静图 | `200..299` | E1 指向这里 |
| Host 获取 IP 中 | `300..399` | host-usb `PENDING_PAGE` |
| Host DHCP 失败 | `400..499` | host-usb `DHCP_FAILED_PAGE` |
| Host IP 背景 | `500..599` | host-usb `IP_BACKGROUND_PAGE` |
| 启动 logo | `3820..3825` | E2, 160x68 mono |
| 数码管字形 | `4026..4037` | 保留官方 N24X33P |
| 资源目录 | `4094` | E0/E1/E2/E3 table |
| Panel config | `4095` | 保留 |
```

明确说明 `3726`、`3826`、`3926` 是旧布局值。新代码不能再使用这些页号，除非是在历史说明或 debug 对比里引用。

- [ ] **步骤 2：更新 flasher notes**

在 `docs/flasher-notes.md` 记录：

```markdown
flasher 现在写入 8 个资源：两帧离线动图、离线静图、获取 IP 中、DHCP 失败、IP 背景、启动 logo、资源目录页。它不再写入 36 帧的 `offline_animation.rgb565be`，也不再擦写 `3726..3825`。
```

- [ ] **步骤 3：更新 host-usb 草案**

在 `docs/host-usb-ip-display-draft.md` 记录：

```markdown
host-usb v1 在紧凑布局后的页号：
- 获取 IP 中：page `300`
- DHCP 失败：page `400`
- IP 背景：page `500`
- 数码管：`4026 + digit`，不变
```

- [ ] **步骤 4：运行文档搜索检查**

运行：

```powershell
rg "3726|3826|3926|offline_animation" docs flasher host-usb
```

期望：剩余命中必须明确是在描述旧布局历史、debug helper 或厂商/出厂布局。活动代码中不能继续引用 `3726`、`3826`、`3926` 或 `offline_animation`。

- [ ] **步骤 5：提交**

```powershell
git add docs/msu2-protocol-and-flash-layout.md docs/flasher-notes.md docs/host-usb-ip-display-draft.md
git commit -m "docs: describe compact flash layout"
```

## 任务 7：完整验证和发布构建

**文件：**
- 本任务没有计划内源码修改。

**接口：**
- 消费：
  - 前面所有任务。
- 产出：
  - 自动化测试通过。
  - 一个可用于人工硬件验证的 flasher 构建产物。

- [ ] **步骤 1：运行 flasher 后端测试**

运行：

```powershell
cargo test --manifest-path flasher/src-tauri/Cargo.toml -- --nocapture
```

期望：全部 flasher 后端测试通过。

- [ ] **步骤 2：运行 host-usb 测试**

运行：

```powershell
cargo test --manifest-path host-usb/Cargo.toml -- --nocapture
```

期望：全部 host-usb 测试通过。

- [ ] **步骤 3：运行 flasher 前端构建**

运行：

```powershell
Set-Location flasher
npm run build
Set-Location ..
```

期望：Vite/TypeScript 构建通过。

- [ ] **步骤 4：构建 flasher 可执行文件**

运行：

```powershell
Set-Location flasher
npm run tauri -- build
Set-Location ..
```

期望：Windows 安装包和可执行构建产物出现在 `flasher/src-tauri/target/release/bundle` 下。

- [ ] **步骤 5：如果默认 target 被正在运行的 flasher 锁住，换 target dir 构建**

运行：

```powershell
$env:CARGO_TARGET_DIR='D:\Work\miniboard\codex-artifacts\compact-flash-layout-target'
Set-Location flasher
npm run tauri -- build
Set-Location ..
Remove-Item Env:\CARGO_TARGET_DIR
```

期望：构建成功，并避开被锁定的默认 `target` 可执行文件。

- [ ] **步骤 6：构建后的人工硬件验收**

用生成的 flasher 刷一次插着的设备：

```text
1. 使用紧凑布局构建版刷写一次。
2. flasher 不关闭、设备不拔出：屏幕保持在 flasher session 对应状态。
3. 关闭 flasher 或拔插设备：离线动图以约 0.9s 可见、0.9s 空白的节奏闪烁。
4. 无 host 连接时按设备按钮：静图模式显示不闪烁的“未连接”图。
5. 设备断电重启：启动 logo 是新的单色 logo，不能是空白。
6. 运行 host-usb pending 状态：设备显示 page 300 的“获取 IP 中”。
7. 运行 host-usb DHCP failed 状态：设备显示 page 400 的 DHCP 失败图。
8. 运行 host-usb IP 状态并使用 255.255.255.255：IP 背景来自 page 500，数字仍从 4026+ 渲染。
```

期望：紧凑动图、离线静图、启动 logo、host 状态图和数码管 IP 渲染都符合布局表。

- [ ] **步骤 7：提交验证中发现的小修正**

如果验证暴露代码或文档问题，按实际改动文件提交：

```powershell
git add flasher/src-tauri/src/assets.rs flasher/src-tauri/src/flasher.rs flasher/tools/generate_flash_assets.py host-usb/src/protocol.rs host-usb/src/display.rs docs/msu2-protocol-and-flash-layout.md docs/flasher-notes.md docs/host-usb-ip-display-draft.md
git commit -m "fix: align compact flash verification"
```

如果验证期间没有文件改动，不要创建这个提交。

## 实现注意事项

- flasher 仍会把相同字节写到不同页范围。例如 `offline.rgb565be` 同时用于 page `0` 和 page `200`，`ip_bg.rgb565be` 同时用于 page `100` 和 page `500`。这是有意的，因为固件按绝对 Flash 页读取，仓库里的文件去重不能减少设备端写入。
- 紧凑布局改变的是设备存储布局，不等于所有仓库资源都天然减少。删除 `offline_animation.rgb565be` 是为了避免以后误用旧 36 帧大资源，并降低仓库二进制资产体积。
- E1 原始 flag 字节是 `00 00 02 00`。实现中不要把 flags 重新解释或规范化成整数后再写；直接使用官方模板字节，只 patch 已确认字段。
- 如果人工硬件验证发现 E0 `count=2` 被固件忽略，只回退资源目录 count/page 策略，其他布局修正仍应保留。后备方案是在不碰 `3651..4095` 的前提下写一个较小的重复闪烁区域，但这个后备方案会牺牲节省空间的收益。

## 自检结果

- 需求覆盖：已覆盖两帧离线动图、释放页空间、host 页号迁移、恢复离线静图、启动 logo 和文档更新。
- 占位检查：没有开放式实现占位。
- 类型一致性：全文统一使用 `FlashAsset`、`page_count`、`PAGE_BYTES`、`RGB_IMAGE_PAGES`，host 页常量统一为 `300/400/500/4026`。
