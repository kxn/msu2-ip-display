use serde::Serialize;

use crate::assets::{
    validate_asset, FlashAsset, HOST_IP_BG_PAGE, HOST_PENDING_PAGE, OFFLINE_VISIBLE_PAGE,
    PAGE_BYTES,
};
use crate::device::PortIo;
use crate::errors::{AppError, AppResult};
use crate::protocol::{
    contains_sequence, erase_flash_pages_packet, expected_write_reply, load_lcd_address_packet,
    set_size_packet, set_xy_packet, show_photo_packet, write_flash_page_packet,
    write_lcd_data_packet,
};
use crate::screen_status::ScreenStatus;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum FlashPhase {
    Erase,
    Write,
    Preview,
    Done,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FlashProgress {
    pub phase: FlashPhase,
    pub current_page: u32,
    pub total_pages: u32,
    pub percent: u8,
    pub display_message: String,
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub attempts: usize,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { attempts: 3 }
    }
}

pub fn flash_images<P, F>(port: &mut P, images: &[FlashAsset<'_>], mut emit: F) -> AppResult<()>
where
    P: PortIo,
    F: FnMut(FlashProgress),
{
    flash_images_internal(port, images, None, &mut emit)
}

pub fn flash_images_with_screen_status<P, F>(
    port: &mut P,
    images: &[FlashAsset<'_>],
    mut emit: F,
) -> AppResult<()>
where
    P: PortIo,
    F: FnMut(FlashProgress),
{
    let mut screen = ScreenStatus::probe(port);
    screen.start(port);
    let result = flash_images_internal(port, images, Some(&mut screen), &mut emit);
    if result.is_ok() {
        screen.finish(port);
    }
    result
}

fn flash_images_internal<P, F>(
    port: &mut P,
    images: &[FlashAsset<'_>],
    mut screen: Option<&mut ScreenStatus>,
    emit: &mut F,
) -> AppResult<()>
where
    P: PortIo,
    F: FnMut(FlashProgress),
{
    for image in images {
        validate_asset(image).map_err(|err| AppError::Asset(err.to_string()))?;
    }

    let total_pages: u32 = images.iter().map(|image| image.page_count as u32).sum();
    let mut completed_pages = 0u32;

    for image in images {
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

            let progress = FlashProgress {
                phase: FlashPhase::Write,
                current_page: completed_pages,
                total_pages,
                percent: ((completed_pages * 100) / total_pages) as u8,
                display_message: format!("写入中 {}%", (completed_pages * 100) / total_pages),
            };
            if let Some(screen) = screen.as_mut() {
                screen.update(port, progress.percent);
            }
            emit(progress);
        }
    }

    emit(FlashProgress {
        phase: FlashPhase::Done,
        current_page: total_pages,
        total_pages,
        percent: 100,
        display_message: "写入完成".to_string(),
    });

    Ok(())
}

fn erase_pages<P: PortIo>(
    port: &mut P,
    start_page: u16,
    page_count: u16,
    retry: RetryPolicy,
) -> AppResult<()> {
    let packet = erase_flash_pages_packet(start_page, page_count);
    for attempt in 1..=retry.attempts {
        port.write_all(&packet)?;
        let reply = port.read_idle(1_200, 40)?;
        if contains_sequence(&reply, &packet) {
            return Ok(());
        }
        if attempt == retry.attempts {
            return Err(AppError::Protocol(format!(
                "erase pages {}..{} expected {:02X?}, got {:02X?}",
                start_page,
                start_page + page_count - 1,
                packet,
                reply
            )));
        }
    }
    Err(AppError::Protocol(
        "unreachable erase retry state".to_string(),
    ))
}

fn write_page<P: PortIo>(
    port: &mut P,
    page: u32,
    data: &[u8; 256],
    retry: RetryPolicy,
) -> AppResult<()> {
    let packet = write_flash_page_packet(page, data);
    let expected = expected_write_reply(page);
    for attempt in 1..=retry.attempts {
        port.write_all(&packet)?;
        let reply = port.read_idle(700, 40)?;
        if contains_sequence(&reply, &expected) {
            return Ok(());
        }
        if attempt == retry.attempts {
            return Err(AppError::Protocol(format!(
                "write page {} expected {:02X?}, got {:02X?}",
                page, expected, reply
            )));
        }
    }
    Err(AppError::Protocol(
        "unreachable write retry state".to_string(),
    ))
}

pub fn write_lcd_region<P: PortIo>(
    port: &mut P,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    bytes: &[u8],
    retry: RetryPolicy,
) -> AppResult<()> {
    let expected_len = (width as usize) * (height as usize) * 2;
    if width == 0 || height == 0 || bytes.len() != expected_len {
        return Err(AppError::Protocol(format!(
            "lcd region {}x{} at {},{} has {} bytes, expected {}",
            width,
            height,
            x,
            y,
            bytes.len(),
            expected_len
        )));
    }

    port.write_all(&set_xy_packet(x, y))?;
    port.write_all(&set_size_packet(width, height))?;

    let address_packet = load_lcd_address_packet();
    for attempt in 1..=retry.attempts {
        port.write_all(&address_packet)?;
        let reply = port.read_idle(300, 40)?;
        if contains_sequence(&reply, &address_packet) {
            break;
        }
        if attempt == retry.attempts {
            return Err(AppError::Protocol(format!(
                "lcd address {},{} {}x{} expected {:02X?}, got {:02X?}",
                x, y, width, height, address_packet, reply
            )));
        }
    }

    for chunk in bytes.chunks(256) {
        let mut page = [0xff; 256];
        page[..chunk.len()].copy_from_slice(chunk);
        port.write_all(&write_lcd_data_packet(chunk.len() as u16, &page))?;
    }

    Ok(())
}

pub fn preview_pages<P: PortIo>(port: &mut P) -> AppResult<()> {
    for page in [
        OFFLINE_VISIBLE_PAGE,
        HOST_PENDING_PAGE,
        HOST_IP_BG_PAGE,
        OFFLINE_VISIBLE_PAGE,
    ] {
        port.write_all(&set_xy_packet(0, 0))?;
        port.read_idle(80, 20)?;
        port.write_all(&set_size_packet(160, 80))?;
        port.read_idle(80, 20)?;
        port.write_all(&show_photo_packet(page))?;
        port.read_idle(120, 20)?;
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{FlashImage, IMAGE_BYTES, PAGES_PER_IMAGE};
    use std::collections::VecDeque;

    struct MockPort {
        writes: Vec<Vec<u8>>,
        replies: VecDeque<Vec<u8>>,
        read_calls: Vec<(u64, u64)>,
    }

    impl MockPort {
        fn new() -> Self {
            Self {
                writes: Vec::new(),
                replies: VecDeque::new(),
                read_calls: Vec::new(),
            }
        }

        fn with_replies(replies: Vec<Vec<u8>>) -> Self {
            Self {
                writes: Vec::new(),
                replies: replies.into(),
                read_calls: Vec::new(),
            }
        }
    }

    impl PortIo for MockPort {
        fn write_all(&mut self, bytes: &[u8]) -> AppResult<()> {
            self.writes.push(bytes.to_vec());
            Ok(())
        }

        fn read_idle(&mut self, total_ms: u64, idle_ms: u64) -> AppResult<Vec<u8>> {
            self.read_calls.push((total_ms, idle_ms));
            if let Some(reply) = self.replies.pop_front() {
                return Ok(reply);
            }
            let Some(last) = self.writes.last() else {
                return Ok(vec![]);
            };
            if last.len() == 6 && last[0] == 0x03 && last[1] == 0x02 {
                return Ok(last.clone());
            }
            if last.len() == 390 && last[384] == 0x03 && last[385] == 0x03 {
                return Ok(last[384..390].to_vec());
            }
            Ok(last.clone())
        }
    }

    #[test]
    fn writes_one_image_as_erase_then_100_pages() {
        let image = vec![0x5a; IMAGE_BYTES];
        let flash_image = FlashImage {
            label: "test",
            start_page: 3826,
            page_count: PAGES_PER_IMAGE,
            bytes: &image,
        };
        let mut port = MockPort::new();
        let mut progress = Vec::new();

        flash_images(&mut port, &[flash_image], |event| progress.push(event)).unwrap();

        assert_eq!(port.writes.len(), 101);
        assert_eq!(port.writes[0], vec![0x03, 0x02, 0x0e, 0xf2, 0x00, 0x64]);
        assert_eq!(
            &port.writes[1][384..390],
            &[0x03, 0x03, 0x00, 0x0e, 0xf2, 0x01]
        );
        assert_eq!(
            &port.writes[100][384..390],
            &[0x03, 0x03, 0x00, 0x0f, 0x55, 0x01]
        );
        assert_eq!(progress.last().unwrap().percent, 100);
    }

    #[test]
    fn writes_variable_sized_assets() {
        let logo = vec![0x5a; PAGE_BYTES * 6];
        let directory = vec![0xa5; PAGE_BYTES];
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
        assert_eq!(
            &port.writes[1][384..390],
            &[0x03, 0x03, 0x00, 0x0e, 0xec, 0x01]
        );
        assert_eq!(
            &port.writes[6][384..390],
            &[0x03, 0x03, 0x00, 0x0e, 0xf1, 0x01]
        );
        assert_eq!(port.writes[7], vec![0x03, 0x02, 0x0f, 0xfe, 0x00, 0x01]);
        assert_eq!(
            &port.writes[8][384..390],
            &[0x03, 0x03, 0x00, 0x0f, 0xfe, 0x01]
        );
        assert_eq!(progress.last().unwrap().percent, 100);
        assert_eq!(progress.last().unwrap().total_pages, 7);
    }

    #[test]
    fn preview_pages_sends_page_zero_last() {
        let mut port = MockPort::new();
        preview_pages(&mut port).unwrap();
        let photo_packets: Vec<Vec<u8>> = port
            .writes
            .iter()
            .filter(|packet| packet.starts_with(&[0x02, 0x03, 0x00]))
            .cloned()
            .collect();
        assert_eq!(photo_packets.len(), 4);
        assert_eq!(photo_packets[0], vec![0x02, 0x03, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(photo_packets[1], vec![0x02, 0x03, 0x00, 0x01, 0x2c, 0x00]);
        assert_eq!(photo_packets[2], vec![0x02, 0x03, 0x00, 0x01, 0xf4, 0x00]);
        assert_eq!(photo_packets[3], vec![0x02, 0x03, 0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn lcd_region_writer_sends_address_and_data_packets() {
        let bytes = vec![0x12; 272];
        let mut port = MockPort::new();

        write_lcd_region(&mut port, 4, 5, 8, 17, &bytes, RetryPolicy::default()).unwrap();

        assert_eq!(port.writes.len(), 5);
        assert_eq!(port.writes[0], vec![0x02, 0x00, 0x00, 0x04, 0x00, 0x05]);
        assert_eq!(port.writes[1], vec![0x02, 0x01, 0x00, 0x08, 0x00, 0x11]);
        assert_eq!(port.writes[2], vec![0x02, 0x03, 0x07, 0x00, 0x00, 0x00]);
        assert_eq!(
            &port.writes[3][384..390],
            &[0x02, 0x03, 0x08, 0x01, 0x00, 0x00]
        );
        assert_eq!(
            &port.writes[4][384..390],
            &[0x02, 0x03, 0x08, 0x00, 0x10, 0x00]
        );
        assert_eq!(
            &port.writes[4][0..24],
            &[
                0x04, 0x00, 0x12, 0x12, 0x12, 0x12, 0x04, 0x01, 0x12, 0x12, 0x12, 0x12, 0x04, 0x02,
                0x12, 0x12, 0x12, 0x12, 0x04, 0x03, 0x12, 0x12, 0x12, 0x12
            ]
        );
        assert_eq!(
            &port.writes[4][24..30],
            &[0x04, 0x04, 0xff, 0xff, 0xff, 0xff]
        );
    }

    #[test]
    fn lcd_region_writer_waits_only_for_lcd_add_ack() {
        let bytes = vec![0x12; 2];
        let mut port = MockPort::new();

        write_lcd_region(&mut port, 4, 5, 1, 1, &bytes, RetryPolicy::default()).unwrap();

        assert_eq!(port.read_calls, vec![(300, 40)]);
    }

    #[test]
    fn lcd_region_writer_rejects_wrong_size_without_writes() {
        let bytes = vec![0x00; 23];
        let mut port = MockPort::new();

        let err =
            write_lcd_region(&mut port, 0, 0, 3, 4, &bytes, RetryPolicy::default()).unwrap_err();

        assert!(matches!(err, AppError::Protocol(_)));
        assert!(port.writes.is_empty());
    }

    #[test]
    fn wrong_size_later_image_causes_zero_writes() {
        let good = vec![0x5a; IMAGE_BYTES];
        let bad = vec![0x5a; IMAGE_BYTES - 1];
        let images = [
            FlashImage {
                label: "good",
                start_page: 3826,
                page_count: PAGES_PER_IMAGE,
                bytes: &good,
            },
            FlashImage {
                label: "bad",
                start_page: 3926,
                page_count: PAGES_PER_IMAGE,
                bytes: &bad,
            },
        ];
        let mut port = MockPort::new();

        let err = flash_images(&mut port, &images, |_| {}).unwrap_err();

        assert!(matches!(err, AppError::Asset(_)));
        assert!(port.writes.is_empty());
    }

    #[test]
    fn retries_a_failed_write_once_then_succeeds() {
        let image = vec![0x5a; IMAGE_BYTES];
        let flash_image = FlashImage {
            label: "test",
            start_page: 3826,
            page_count: PAGES_PER_IMAGE,
            bytes: &image,
        };
        let first_page = flash_image.start_page as u32;
        let mut port = MockPort::with_replies(vec![
            erase_flash_pages_packet(flash_image.start_page, PAGES_PER_IMAGE).to_vec(),
            vec![0xde, 0xad],
            expected_write_reply(first_page).to_vec(),
        ]);

        flash_images(&mut port, &[flash_image], |_| {}).unwrap();

        assert_eq!(port.writes.len(), 102);
        assert_eq!(
            &port.writes[1][384..390],
            &[0x03, 0x03, 0x00, 0x0e, 0xf2, 0x01]
        );
        assert_eq!(
            &port.writes[2][384..390],
            &[0x03, 0x03, 0x00, 0x0e, 0xf2, 0x01]
        );
    }

    #[test]
    fn fails_after_exhausting_write_retries() {
        let image = vec![0x5a; IMAGE_BYTES];
        let flash_image = FlashImage {
            label: "test",
            start_page: 3826,
            page_count: PAGES_PER_IMAGE,
            bytes: &image,
        };
        let mut port = MockPort::with_replies(vec![
            erase_flash_pages_packet(flash_image.start_page, PAGES_PER_IMAGE).to_vec(),
            vec![0xde, 0xad],
            vec![0xbe, 0xef],
            vec![0xfa, 0xce],
        ]);

        let err = flash_images(&mut port, &[flash_image], |_| {}).unwrap_err();

        assert!(matches!(err, AppError::Protocol(_)));
        assert_eq!(port.writes.len(), 4);
    }

    #[test]
    fn screen_status_probe_failure_does_not_fail_flash() {
        let image = vec![0x5a; IMAGE_BYTES];
        let flash_image = FlashImage {
            label: "test",
            start_page: 3826,
            page_count: PAGES_PER_IMAGE,
            bytes: &image,
        };
        let mut port = MockPort::with_replies(vec![vec![], vec![], vec![0xde, 0xad]]);
        let mut progress = Vec::new();

        flash_images_with_screen_status(&mut port, &[flash_image], |event| progress.push(event))
            .unwrap();

        assert_eq!(progress.last().unwrap().percent, 100);
        assert!(port
            .writes
            .iter()
            .any(|packet| packet == &erase_flash_pages_packet(3826, PAGES_PER_IMAGE).to_vec()));
    }
}
