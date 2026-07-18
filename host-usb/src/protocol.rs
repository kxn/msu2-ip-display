pub const HANDSHAKE: [u8; 6] = [0x00, b'M', b'S', b'N', b'C', b'N'];
pub const SCREEN_WIDTH: u16 = 160;
pub const SCREEN_HEIGHT: u16 = 80;
pub const DHCP_FAILED_PAGE: u16 = 400;
pub const PENDING_PAGE: u16 = 300;
pub const IP_BACKGROUND_PAGE: u16 = 500;
pub const DIGIT_RESOURCE_PAGE: u16 = 4026;

#[inline]
fn hi16(value: u16) -> u8 {
    ((value >> 8) & 0xff) as u8
}

#[inline]
fn lo16(value: u16) -> u8 {
    (value & 0xff) as u8
}

#[inline]
fn addr_hi(value: u32) -> u8 {
    ((value >> 16) & 0xff) as u8
}

#[inline]
fn addr_mid(value: u32) -> u8 {
    ((value >> 8) & 0xff) as u8
}

#[inline]
fn addr_lo(value: u32) -> u8 {
    (value & 0xff) as u8
}

pub fn set_xy_packet(x: u16, y: u16) -> [u8; 6] {
    [0x02, 0x00, hi16(x), lo16(x), hi16(y), lo16(y)]
}

pub fn set_size_packet(width: u16, height: u16) -> [u8; 6] {
    [
        0x02,
        0x01,
        hi16(width),
        lo16(width),
        hi16(height),
        lo16(height),
    ]
}

pub fn set_color_packet(foreground: u16, background: u16) -> [u8; 6] {
    [
        0x02,
        0x02,
        hi16(foreground),
        lo16(foreground),
        hi16(background),
        lo16(background),
    ]
}

pub fn show_photo_packet(page: u16) -> [u8; 6] {
    [0x02, 0x03, 0x00, hi16(page), lo16(page), 0x00]
}

pub fn ram_init_packet(fill: u8) -> [u8; 6] {
    [0x02, 0x03, 0x0d, fill, 0x00, 0x00]
}

pub fn add_ram_masked_packet(address: u32) -> [u8; 6] {
    [
        0x02,
        0x03,
        0x0f,
        addr_hi(address),
        addr_mid(address),
        addr_lo(address),
    ]
}

pub fn load_ram_mix_show_packet(background_page: u16) -> [u8; 6] {
    let address = background_page as u32 * 256;
    [
        0x02,
        0x03,
        0x11,
        addr_hi(address),
        addr_mid(address),
        addr_lo(address),
    ]
}

pub fn load_lcd_address_packet() -> [u8; 6] {
    [0x02, 0x03, 0x07, 0x00, 0x00, 0x00]
}

pub fn write_lcd_data_packet(size: u16, data: &[u8; 256]) -> [u8; 390] {
    let mut packet = [0u8; 390];

    for index in 0..64usize {
        let src = index * 4;
        let dst = index * 6;
        packet[dst] = 0x04;
        packet[dst + 1] = index as u8;
        packet[dst + 2] = data[src];
        packet[dst + 3] = data[src + 1];
        packet[dst + 4] = data[src + 2];
        packet[dst + 5] = data[src + 3];
    }

    packet[384] = 0x02;
    packet[385] = 0x03;
    packet[386] = 0x08;
    packet[387] = hi16(size);
    packet[388] = lo16(size);
    packet[389] = 0x00;
    packet
}

pub fn contains_sequence(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack.len() >= needle.len()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_packets_match_verified_protocol() {
        assert_eq!(HANDSHAKE, [0x00, 0x4d, 0x53, 0x4e, 0x43, 0x4e]);
        assert_eq!(set_xy_packet(0, 0), [0x02, 0x00, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(
            set_size_packet(160, 80),
            [0x02, 0x01, 0x00, 0xa0, 0x00, 0x50]
        );
        assert_eq!(
            show_photo_packet(IP_BACKGROUND_PAGE),
            [0x02, 0x03, 0x00, 0x01, 0xf4, 0x00]
        );
    }

    #[test]
    fn official_digit_ram_packets_match_hardware_probe() {
        assert_eq!(ram_init_packet(0), [0x02, 0x03, 0x0d, 0x00, 0x00, 0x00]);
        assert_eq!(
            add_ram_masked_packet((4026 + 5) * 256),
            [0x02, 0x03, 0x0f, 0x0f, 0xbf, 0x00]
        );
        assert_eq!(
            load_ram_mix_show_packet(IP_BACKGROUND_PAGE),
            [0x02, 0x03, 0x11, 0x01, 0xf4, 0x00]
        );
    }

    #[test]
    fn compact_layout_page_packets_match_expected_bytes() {
        assert_eq!(
            show_photo_packet(PENDING_PAGE),
            [0x02, 0x03, 0x00, 0x01, 0x2c, 0x00]
        );
        assert_eq!(
            show_photo_packet(DHCP_FAILED_PAGE),
            [0x02, 0x03, 0x00, 0x01, 0x90, 0x00]
        );
        assert_eq!(
            show_photo_packet(IP_BACKGROUND_PAGE),
            [0x02, 0x03, 0x00, 0x01, 0xf4, 0x00]
        );
        assert_eq!(
            load_ram_mix_show_packet(IP_BACKGROUND_PAGE),
            [0x02, 0x03, 0x11, 0x01, 0xf4, 0x00]
        );
    }

    #[test]
    fn lcd_direct_write_packet_matches_flasher() {
        assert_eq!(
            load_lcd_address_packet(),
            [0x02, 0x03, 0x07, 0x00, 0x00, 0x00]
        );
        let data = [0x5a; 256];
        let packet = write_lcd_data_packet(16, &data);
        assert_eq!(packet.len(), 390);
        assert_eq!(&packet[0..6], &[0x04, 0x00, 0x5a, 0x5a, 0x5a, 0x5a]);
        assert_eq!(&packet[384..390], &[0x02, 0x03, 0x08, 0x00, 0x10, 0x00]);
    }

    #[test]
    fn contains_sequence_finds_reply_inside_buffer() {
        assert!(contains_sequence(
            &[0xaa, 0x00, 0x4d, 0x53, 0x4e, 0x43, 0x4e, 0xbb],
            &HANDSHAKE,
        ));
        assert!(!contains_sequence(&[0x00, 0x4d, 0x53], &HANDSHAKE));
    }
}
