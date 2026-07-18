use std::net::Ipv4Addr;

use qrcodegen::{QrCode, QrCodeEcc};

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 80;
pub const QR_WHITE: u16 = 0xffff;
pub const QR_BLACK: u16 = 0x0000;

const QR_SCALE: usize = 2;
const QR_BORDER_MODULES: usize = 4;
const MAX_QR_VERSION: u8 = 3;

pub fn validate_template(template: &str) -> Result<(), String> {
    if !template.contains("{ip}") {
        return Err("QR template must contain {ip}".to_string());
    }

    let _ = render_qr_rgb565be(template, Ipv4Addr::new(255, 255, 255, 255))?;
    Ok(())
}

pub fn render_qr_rgb565be(template: &str, ip: Ipv4Addr) -> Result<Vec<u8>, String> {
    if !template.contains("{ip}") {
        return Err("QR template must contain {ip}".to_string());
    }

    let content = template.replace("{ip}", &ip.to_string());
    let qr = QrCode::encode_text(&content, QrCodeEcc::Medium)
        .map_err(|_| "QR URL too long for this screen".to_string())?;
    let version = qr.version().value();
    if version > MAX_QR_VERSION {
        return Err(format!(
            "QR URL too long for this screen: version {version} exceeds version {MAX_QR_VERSION}"
        ));
    }

    let qr_size = qr.size() as usize;
    let side = (qr_size + QR_BORDER_MODULES * 2) * QR_SCALE;
    if side > SCREEN_HEIGHT || side > SCREEN_WIDTH {
        return Err(format!(
            "QR URL too long for this screen: rendered size {side}px exceeds display"
        ));
    }

    let x0 = (SCREEN_WIDTH - side) / 2;
    let y0 = (SCREEN_HEIGHT - side) / 2;
    let mut bytes = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 2];
    fill(&mut bytes, QR_WHITE);

    for y in 0..side {
        for x in 0..side {
            let module_x = x / QR_SCALE;
            let module_y = y / QR_SCALE;
            if module_x < QR_BORDER_MODULES || module_y < QR_BORDER_MODULES {
                continue;
            }

            let qr_x = module_x - QR_BORDER_MODULES;
            let qr_y = module_y - QR_BORDER_MODULES;
            if qr_x >= qr_size || qr_y >= qr_size {
                continue;
            }

            if qr.get_module(qr_x as i32, qr_y as i32) {
                set_pixel(&mut bytes, x0 + x, y0 + y, QR_BLACK);
            }
        }
    }

    Ok(bytes)
}

fn fill(bytes: &mut [u8], color: u16) {
    let encoded = color.to_be_bytes();
    for pixel in bytes.chunks_exact_mut(2) {
        pixel.copy_from_slice(&encoded);
    }
}

fn set_pixel(bytes: &mut [u8], x: usize, y: usize, color: u16) {
    let offset = (y * SCREEN_WIDTH + x) * 2;
    bytes[offset..offset + 2].copy_from_slice(&color.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_default_template() {
        validate_template("http://{ip}/").unwrap();
    }

    #[test]
    fn rejects_template_without_ip_placeholder() {
        let err = validate_template("http://device.local/").unwrap_err();
        assert!(err.contains("{ip}"));
    }

    #[test]
    fn rejects_template_that_exceeds_version_three() {
        let long_template = format!("http://{{ip}}:8080/{}", "a".repeat(80));
        let err = validate_template(&long_template).unwrap_err();
        assert!(err.contains("too long"));
    }

    #[test]
    fn renders_full_screen_white_background_with_black_modules() {
        let bytes = render_qr_rgb565be("http://{ip}/", Ipv4Addr::new(10, 0, 0, 5)).unwrap();
        assert_eq!(bytes.len(), 160 * 80 * 2);
        assert_eq!(&bytes[0..2], &QR_WHITE.to_be_bytes());
        assert!(bytes.chunks_exact(2).any(|pixel| pixel == [0x00, 0x00]));
    }
}
