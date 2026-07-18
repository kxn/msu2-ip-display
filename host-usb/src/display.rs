use std::net::Ipv4Addr;

use crate::protocol::{
    add_ram_masked_packet, load_lcd_address_packet, load_ram_mix_show_packet, ram_init_packet,
    set_color_packet, set_size_packet, set_xy_packet, show_photo_packet, write_lcd_data_packet,
    DHCP_FAILED_PAGE, DIGIT_RESOURCE_PAGE, IP_BACKGROUND_PAGE, PENDING_PAGE,
};
use crate::qr_display::{render_qr_rgb565be, QR_WHITE};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireWrite {
    pub bytes: Vec<u8>,
    pub wait_for_echo: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DigitGlyph {
    pub x: u16,
    pub y: u16,
    pub digit: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DotGlyph {
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpLayout {
    pub digits: Vec<DigitGlyph>,
    pub dots: Vec<DotGlyph>,
}

pub struct DisplayRenderer;

const DIGIT_WIDTH: u16 = 24;
const DIGIT_HEIGHT: u16 = 33;
const DOT_SLOT_WIDTH: u16 = 8;
const DOT_SIZE: u16 = 5;
const ROW_GAP: u16 = 8;
const SCREEN_WIDTH: u16 = 160;
const SCREEN_HEIGHT: u16 = 80;
const RGB565_TEXT: u16 = 0x5fd0;
const RGB565_BLACK: u16 = 0x0000;
const KEEPALIVE_X: u16 = SCREEN_WIDTH - 1;
const KEEPALIVE_Y: u16 = SCREEN_HEIGHT - 1;

impl DisplayRenderer {
    pub fn pending() -> Vec<WireWrite> {
        full_screen_page(PENDING_PAGE)
    }

    pub fn dhcp_failed() -> Vec<WireWrite> {
        full_screen_page(DHCP_FAILED_PAGE)
    }

    pub fn ip(ip: Ipv4Addr) -> Vec<WireWrite> {
        let mut writes = vec![
            packet(show_photo_packet(IP_BACKGROUND_PAGE), false),
            packet(ram_init_packet(0), false),
        ];

        let layout = Self::layout_ip(ip);
        for glyph in layout.digits {
            let address = (DIGIT_RESOURCE_PAGE as u32 + glyph.digit as u32) * 256;
            writes.push(packet(set_xy_packet(glyph.x, glyph.y), false));
            writes.push(packet(set_size_packet(DIGIT_WIDTH, DIGIT_HEIGHT), false));
            writes.push(packet(add_ram_masked_packet(address), false));
        }

        writes.push(packet(set_color_packet(RGB565_TEXT, RGB565_BLACK), false));
        writes.push(packet(load_ram_mix_show_packet(IP_BACKGROUND_PAGE), false));

        for dot in layout.dots {
            writes.extend(dot_writes(dot));
        }

        writes
    }

    pub fn qr(ip: Ipv4Addr, template: &str) -> Result<Vec<WireWrite>, String> {
        let bytes = render_qr_rgb565be(template, ip)?;
        Ok(lcd_region_writes(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT, &bytes))
    }

    pub fn keepalive() -> Vec<WireWrite> {
        pixel_writes(KEEPALIVE_X, KEEPALIVE_Y, RGB565_BLACK)
    }

    pub fn keepalive_white() -> Vec<WireWrite> {
        pixel_writes(KEEPALIVE_X, KEEPALIVE_Y, QR_WHITE)
    }

    pub fn layout_ip(ip: Ipv4Addr) -> IpLayout {
        let octets = ip.octets();
        let rows = [
            (octets[0].to_string(), octets[1].to_string()),
            (octets[2].to_string(), octets[3].to_string()),
        ];
        let total_height = DIGIT_HEIGHT * 2 + ROW_GAP;
        let start_y = (SCREEN_HEIGHT - total_height) / 2;
        let mut digits = Vec::new();
        let mut dots = Vec::new();

        for (row_index, (left, right)) in rows.iter().enumerate() {
            let y = start_y + row_index as u16 * (DIGIT_HEIGHT + ROW_GAP);
            let row = format!("{left}.{right}");
            let mut x = (SCREEN_WIDTH - row_width(&row)) / 2;

            for ch in row.chars() {
                if ch == '.' {
                    dots.push(DotGlyph {
                        x: x + (DOT_SLOT_WIDTH - DOT_SIZE) / 2,
                        y: y + DIGIT_HEIGHT - DOT_SIZE - 3,
                    });
                    x += DOT_SLOT_WIDTH;
                } else {
                    let digit = ch
                        .to_digit(10)
                        .expect("IPv4 rows contain only digits and dot");
                    digits.push(DigitGlyph {
                        x,
                        y,
                        digit: digit as u8,
                    });
                    x += DIGIT_WIDTH;
                }
            }
        }

        IpLayout { digits, dots }
    }
}

fn full_screen_page(page: u16) -> Vec<WireWrite> {
    vec![
        packet(set_xy_packet(0, 0), false),
        packet(set_size_packet(SCREEN_WIDTH, SCREEN_HEIGHT), false),
        packet(show_photo_packet(page), false),
    ]
}

fn lcd_region_writes(x: u16, y: u16, width: u16, height: u16, bytes: &[u8]) -> Vec<WireWrite> {
    assert_eq!(bytes.len(), width as usize * height as usize * 2);

    let mut writes = vec![
        packet(set_xy_packet(x, y), false),
        packet(set_size_packet(width, height), false),
        packet(load_lcd_address_packet(), true),
    ];

    for chunk in bytes.chunks(256) {
        let mut page = [0u8; 256];
        page[..chunk.len()].copy_from_slice(chunk);
        writes.push(WireWrite {
            bytes: write_lcd_data_packet(chunk.len() as u16, &page).to_vec(),
            wait_for_echo: false,
        });
    }

    writes
}

fn packet(bytes: [u8; 6], wait_for_echo: bool) -> WireWrite {
    WireWrite {
        bytes: bytes.to_vec(),
        wait_for_echo,
    }
}

fn row_width(row: &str) -> u16 {
    row.chars()
        .map(|ch| {
            if ch == '.' {
                DOT_SLOT_WIDTH
            } else {
                DIGIT_WIDTH
            }
        })
        .sum()
}

fn dot_writes(dot: DotGlyph) -> Vec<WireWrite> {
    let mut dot_bytes = [0u8; 256];
    for pixel in dot_bytes
        .chunks_exact_mut(2)
        .take((DOT_SIZE as usize) * (DOT_SIZE as usize))
    {
        pixel.copy_from_slice(&RGB565_TEXT.to_be_bytes());
    }

    vec![
        packet(set_xy_packet(dot.x, dot.y), false),
        packet(set_size_packet(DOT_SIZE, DOT_SIZE), false),
        packet(load_lcd_address_packet(), true),
        WireWrite {
            bytes: write_lcd_data_packet(DOT_SIZE * DOT_SIZE * 2, &dot_bytes).to_vec(),
            wait_for_echo: false,
        },
    ]
}

fn pixel_writes(x: u16, y: u16, color: u16) -> Vec<WireWrite> {
    let mut pixel_bytes = [0u8; 256];
    pixel_bytes[0..2].copy_from_slice(&color.to_be_bytes());

    vec![
        packet(set_xy_packet(x, y), false),
        packet(set_size_packet(1, 1), false),
        packet(load_lcd_address_packet(), true),
        WireWrite {
            bytes: write_lcd_data_packet(2, &pixel_bytes).to_vec(),
            wait_for_echo: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_state_commands_show_expected_pages() {
        assert_eq!(
            DisplayRenderer::pending().last().unwrap().bytes,
            show_photo_packet(PENDING_PAGE).to_vec()
        );
        assert_eq!(
            DisplayRenderer::dhcp_failed().last().unwrap().bytes,
            show_photo_packet(DHCP_FAILED_PAGE).to_vec()
        );
    }

    #[test]
    fn page_state_commands_set_fullscreen_window_before_showing_page() {
        let pending = DisplayRenderer::pending();
        assert_eq!(pending.len(), 3);
        assert_eq!(pending[0].bytes, set_xy_packet(0, 0).to_vec());
        assert_eq!(
            pending[1].bytes,
            set_size_packet(SCREEN_WIDTH, SCREEN_HEIGHT).to_vec()
        );
        assert_eq!(pending[2].bytes, show_photo_packet(PENDING_PAGE).to_vec());

        let dhcp_failed = DisplayRenderer::dhcp_failed();
        assert_eq!(dhcp_failed.len(), 3);
        assert_eq!(dhcp_failed[0].bytes, set_xy_packet(0, 0).to_vec());
        assert_eq!(
            dhcp_failed[1].bytes,
            set_size_packet(SCREEN_WIDTH, SCREEN_HEIGHT).to_vec()
        );
        assert_eq!(
            dhcp_failed[2].bytes,
            show_photo_packet(DHCP_FAILED_PAGE).to_vec()
        );
    }

    #[test]
    fn max_ip_layout_is_centered_in_two_rows() {
        let layout = DisplayRenderer::layout_ip(Ipv4Addr::new(255, 255, 255, 255));
        assert_eq!(layout.digits.len(), 12);
        assert_eq!(layout.dots.len(), 2);
        assert_eq!(
            layout.digits[0],
            DigitGlyph {
                x: 4,
                y: 3,
                digit: 2
            }
        );
        assert_eq!(
            layout.digits[5],
            DigitGlyph {
                x: 132,
                y: 3,
                digit: 5
            }
        );
        assert_eq!(
            layout.digits[6],
            DigitGlyph {
                x: 4,
                y: 44,
                digit: 2
            }
        );
        assert_eq!(layout.dots[0], DotGlyph { x: 77, y: 28 });
        assert_eq!(layout.dots[1], DotGlyph { x: 77, y: 69 });
    }

    #[test]
    fn short_ip_rows_are_independently_centered() {
        let layout = DisplayRenderer::layout_ip(Ipv4Addr::new(10, 0, 1, 5));
        assert_eq!(layout.digits[0].x, 40);
        assert_eq!(layout.digits[2].x, 96);
        assert_eq!(layout.digits[3].x, 52);
        assert_eq!(layout.digits[4].x, 84);
        assert_eq!(layout.dots[0].x, 89);
        assert_eq!(layout.dots[1].x, 77);
    }

    #[test]
    fn ip_render_starts_with_background_and_loads_ram_mix() {
        let writes = DisplayRenderer::ip(Ipv4Addr::new(192, 168, 1, 204));
        assert_eq!(
            writes[0].bytes,
            show_photo_packet(IP_BACKGROUND_PAGE).to_vec()
        );
        assert!(writes
            .iter()
            .any(|write| write.bytes == [0x02, 0x03, 0x0d, 0x00, 0x00, 0x00]));
        assert!(writes
            .iter()
            .any(|write| write.bytes == [0x02, 0x03, 0x11, 0x01, 0xf4, 0x00]));
    }

    #[test]
    fn ip_render_uses_status_text_color_for_digits_and_dots() {
        const STATUS_TEXT_RGB565: u16 = 0x5fd0;
        let writes = DisplayRenderer::ip(Ipv4Addr::new(10, 0, 1, 5));
        assert!(writes.iter().any(|write| {
            write.bytes == set_color_packet(STATUS_TEXT_RGB565, RGB565_BLACK).to_vec()
        }));

        let dot_write = writes
            .iter()
            .find(|write| write.bytes.len() == 390)
            .expect("expected direct dot pixel write");
        assert_eq!(
            &dot_write.bytes[2..4],
            &STATUS_TEXT_RGB565.to_be_bytes(),
            "dot pixels should use the same visible text color as the RAM digit overlay"
        );
    }

    #[test]
    fn keepalive_dot_stays_within_screen_bounds() {
        let writes = DisplayRenderer::keepalive();
        assert_eq!(writes[0].bytes, set_xy_packet(159, 79).to_vec());
        assert_eq!(writes[1].bytes, set_size_packet(1, 1).to_vec());
        assert!(159 < SCREEN_WIDTH);
        assert!(79 < SCREEN_HEIGHT);
    }

    #[test]
    fn keepalive_writes_only_one_pixel() {
        let writes = DisplayRenderer::keepalive();
        assert_eq!(writes[1].bytes, set_size_packet(1, 1).to_vec());

        let lcd_write = writes
            .iter()
            .find(|write| write.bytes.len() == 390)
            .expect("expected lcd data write");
        assert_eq!(
            &lcd_write.bytes[384..390],
            &[0x02, 0x03, 0x08, 0x00, 0x02, 0x00]
        );

        let mut pixel_bytes = Vec::new();
        for chunk in lcd_write.bytes[..384].chunks_exact(6) {
            pixel_bytes.extend_from_slice(&chunk[2..6]);
        }
        assert_eq!(&pixel_bytes[0..2], &[0x00, 0x00]);
        assert!(pixel_bytes[2..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn qr_render_writes_full_screen_lcd_region() {
        let writes = DisplayRenderer::qr(Ipv4Addr::new(10, 0, 0, 5), "http://{ip}/").unwrap();
        assert_eq!(writes[0].bytes, set_xy_packet(0, 0).to_vec());
        assert_eq!(
            writes[1].bytes,
            set_size_packet(SCREEN_WIDTH, SCREEN_HEIGHT).to_vec()
        );
        assert_eq!(writes[2].bytes, load_lcd_address_packet().to_vec());
        assert_eq!(writes.len(), 103);
    }

    #[test]
    fn qr_keepalive_uses_white_corner_pixel() {
        let writes = DisplayRenderer::keepalive_white();
        let lcd_write = writes
            .iter()
            .find(|write| write.bytes.len() == 390)
            .expect("expected lcd data write");
        assert_eq!(&lcd_write.bytes[2..4], &[0xff, 0xff]);
    }
}
