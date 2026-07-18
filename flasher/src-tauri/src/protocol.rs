pub const HANDSHAKE: [u8; 6] = [0x00, b'M', b'S', b'N', b'C', b'N'];

#[inline]
fn hi(value: u16) -> u8 {
    ((value >> 8) & 0xff) as u8
}

#[inline]
fn lo(value: u16) -> u8 {
    (value & 0xff) as u8
}

pub fn erase_flash_pages_packet(start_page: u16, page_count: u16) -> [u8; 6] {
    [
        0x03,
        0x02,
        hi(start_page),
        lo(start_page),
        hi(page_count),
        lo(page_count),
    ]
}

pub fn write_flash_page_packet(page: u32, data: &[u8; 256]) -> [u8; 390] {
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

    packet[384] = 0x03;
    packet[385] = 0x03;
    packet[386] = ((page >> 16) & 0xff) as u8;
    packet[387] = ((page >> 8) & 0xff) as u8;
    packet[388] = (page & 0xff) as u8;
    packet[389] = 0x01;
    packet
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
    packet[387] = hi(size);
    packet[388] = lo(size);
    packet[389] = 0x00;
    packet
}

pub fn expected_write_reply(page: u32) -> [u8; 6] {
    [
        0x03,
        0x03,
        ((page >> 16) & 0xff) as u8,
        ((page >> 8) & 0xff) as u8,
        (page & 0xff) as u8,
        0x01,
    ]
}

pub fn set_xy_packet(x: u16, y: u16) -> [u8; 6] {
    [0x02, 0x00, hi(x), lo(x), hi(y), lo(y)]
}

pub fn set_size_packet(width: u16, height: u16) -> [u8; 6] {
    [0x02, 0x01, hi(width), lo(width), hi(height), lo(height)]
}

pub fn show_photo_packet(page: u16) -> [u8; 6] {
    [0x02, 0x03, 0x00, hi(page), lo(page), 0x00]
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
    use crate::assets::{HOST_IP_BG_PAGE, HOST_PENDING_PAGE};

    use super::*;

    #[test]
    fn handshake_matches_verified_bytes() {
        assert_eq!(HANDSHAKE, [0x00, 0x4d, 0x53, 0x4e, 0x43, 0x4e]);
    }

    #[test]
    fn erase_packet_encodes_compact_pending_range() {
        assert_eq!(
            erase_flash_pages_packet(HOST_PENDING_PAGE, 100),
            [0x03, 0x02, 0x01, 0x2c, 0x00, 0x64]
        );
    }

    #[test]
    fn write_page_packet_has_compact_layout_footer() {
        let data = [0x5a; 256];
        let packet = write_flash_page_packet(HOST_IP_BG_PAGE.into(), &data);
        assert_eq!(packet.len(), 390);
        assert_eq!(&packet[0..6], &[0x04, 0x00, 0x5a, 0x5a, 0x5a, 0x5a]);
        assert_eq!(&packet[378..384], &[0x04, 0x3f, 0x5a, 0x5a, 0x5a, 0x5a]);
        assert_eq!(&packet[384..390], &[0x03, 0x03, 0x00, 0x01, 0xf4, 0x01]);
    }

    #[test]
    fn show_photo_page_zero_matches_verified_bytes() {
        assert_eq!(set_xy_packet(0, 0), [0x02, 0x00, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(
            set_size_packet(160, 80),
            [0x02, 0x01, 0x00, 0xa0, 0x00, 0x50]
        );
        assert_eq!(show_photo_packet(0), [0x02, 0x03, 0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn lcd_direct_write_packets_match_official_demo() {
        assert_eq!(
            load_lcd_address_packet(),
            [0x02, 0x03, 0x07, 0x00, 0x00, 0x00]
        );

        let data = [0x5a; 256];
        let packet = write_lcd_data_packet(16, &data);
        assert_eq!(packet.len(), 390);
        assert_eq!(&packet[0..6], &[0x04, 0x00, 0x5a, 0x5a, 0x5a, 0x5a]);
        assert_eq!(&packet[378..384], &[0x04, 0x3f, 0x5a, 0x5a, 0x5a, 0x5a]);
        assert_eq!(&packet[384..390], &[0x02, 0x03, 0x08, 0x00, 0x10, 0x00]);
    }

    #[test]
    fn contains_sequence_finds_reply_inside_buffer() {
        assert!(contains_sequence(
            &[0xaa, 0x03, 0x03, 0x00, 0x0f, 0x56, 0x01, 0xbb],
            &[0x03, 0x03, 0x00, 0x0f, 0x56, 0x01],
        ));
        assert!(!contains_sequence(
            &[0x03, 0x03, 0x00],
            &[0x03, 0x03, 0x00, 0x0f]
        ));
    }
}
