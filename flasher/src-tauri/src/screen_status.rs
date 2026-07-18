use crate::device::PortIo;
use crate::errors::AppResult;
use crate::flasher::{write_lcd_region, RetryPolicy};

const GREEN: (u8, u8, u8) = (51, 255, 127);
const SCREEN_WIDTH: u16 = 160;
const SCREEN_HEIGHT: u16 = 80;
const PROGRESS_INNER_X: u16 = 26;
const PROGRESS_INNER_Y: u16 = 62;
const PROGRESS_INNER_WIDTH: u16 = 108;
const PROGRESS_INNER_HEIGHT: u16 = 4;
const PERCENT_X: u16 = 62;
const PERCENT_Y: u16 = 42;
const PERCENT_WIDTH: u16 = 36;
const PERCENT_HEIGHT: u16 = 13;
const PERCENT_BYTES: usize = (PERCENT_WIDTH as usize) * (PERCENT_HEIGHT as usize) * 2;
const INITIAL_SCREEN: &[u8] = include_bytes!("../assets/flash_status_screen_initial.rgb565be");
const DONE_SCREEN: &[u8] = include_bytes!("../assets/flash_status_screen_done.rgb565be");
const PERCENT_STRIP: &[u8] = include_bytes!("../assets/flash_status_percent_strip.rgb565be");
const WAITING_SCREEN: &[u8] = include_bytes!("../assets/waiting_to_flash.rgb565be");

struct RegionAsset {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    bytes: &'static [u8],
}

const FULL_INITIAL_SCREEN: RegionAsset = RegionAsset {
    x: 0,
    y: 0,
    width: SCREEN_WIDTH,
    height: SCREEN_HEIGHT,
    bytes: INITIAL_SCREEN,
};

const FULL_DONE_SCREEN: RegionAsset = RegionAsset {
    x: 0,
    y: 0,
    width: SCREEN_WIDTH,
    height: SCREEN_HEIGHT,
    bytes: DONE_SCREEN,
};

const FULL_WAITING_SCREEN: RegionAsset = RegionAsset {
    x: 0,
    y: 0,
    width: SCREEN_WIDTH,
    height: SCREEN_HEIGHT,
    bytes: WAITING_SCREEN,
};

pub fn show_waiting_to_flash<P: PortIo>(port: &mut P) -> AppResult<()> {
    write_full_screen(port, &FULL_WAITING_SCREEN)
}

pub fn show_flash_done<P: PortIo>(port: &mut P) -> AppResult<()> {
    write_full_screen(port, &FULL_DONE_SCREEN)
}

pub fn keepalive_pixel<P: PortIo>(port: &mut P) -> AppResult<()> {
    write_lcd_region(
        port,
        159,
        79,
        1,
        1,
        &[0x00, 0x00],
        RetryPolicy { attempts: 1 },
    )
}

fn write_full_screen<P: PortIo>(port: &mut P, asset: &RegionAsset) -> AppResult<()> {
    write_lcd_region(
        port,
        asset.x,
        asset.y,
        asset.width,
        asset.height,
        asset.bytes,
        RetryPolicy { attempts: 1 },
    )?;
    port.read_idle(250, 40)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenStatus {
    enabled: bool,
    last_percent: Option<u8>,
    last_fill_px: u16,
}

impl ScreenStatus {
    pub fn probe<P: PortIo>(port: &mut P) -> Self {
        let mut status = Self {
            enabled: true,
            last_percent: None,
            last_fill_px: 0,
        };
        let black_pixel = [0x00, 0x00];
        status.write_region(port, 80, 79, 1, 1, &black_pixel);
        status
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn start<P: PortIo>(&mut self, port: &mut P) {
        self.write_asset(port, &FULL_INITIAL_SCREEN);
        if self.enabled {
            self.last_percent = Some(0);
            self.last_fill_px = 0;
        }
    }

    pub fn update<P: PortIo>(&mut self, port: &mut P, percent: u8) {
        if !self.enabled {
            return;
        }

        let percent = percent.min(100);
        if self.last_percent == Some(percent) {
            return;
        }

        self.write_region(
            port,
            PERCENT_X,
            PERCENT_Y,
            PERCENT_WIDTH,
            PERCENT_HEIGHT,
            percent_panel(percent),
        );
        if !self.enabled {
            return;
        }

        let next_fill_px = fill_pixels(percent);
        if next_fill_px > self.last_fill_px {
            let delta = next_fill_px - self.last_fill_px;
            let fill = solid_rgb565(delta, PROGRESS_INNER_HEIGHT, GREEN);
            self.write_region(
                port,
                PROGRESS_INNER_X + self.last_fill_px,
                PROGRESS_INNER_Y,
                delta,
                PROGRESS_INNER_HEIGHT,
                &fill,
            );
            if !self.enabled {
                return;
            }
            self.last_fill_px = next_fill_px;
        }

        self.last_percent = Some(percent);
    }

    pub fn finish<P: PortIo>(&mut self, port: &mut P) {
        self.write_asset(port, &FULL_DONE_SCREEN);
    }

    fn write_asset<P: PortIo>(&mut self, port: &mut P, asset: &RegionAsset) {
        self.write_region(
            port,
            asset.x,
            asset.y,
            asset.width,
            asset.height,
            asset.bytes,
        );
    }

    fn write_region<P: PortIo>(
        &mut self,
        port: &mut P,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        bytes: &[u8],
    ) {
        if !self.enabled {
            return;
        }
        if write_lcd_region(
            port,
            x,
            y,
            width,
            height,
            bytes,
            RetryPolicy { attempts: 1 },
        )
        .is_err()
        {
            self.enabled = false;
        }
    }
}

fn percent_panel(percent: u8) -> &'static [u8] {
    let start = (percent.min(100) as usize) * PERCENT_BYTES;
    &PERCENT_STRIP[start..start + PERCENT_BYTES]
}

fn fill_pixels(percent: u8) -> u16 {
    ((PROGRESS_INNER_WIDTH as u32 * percent.min(100) as u32) / 100) as u16
}

fn solid_rgb565(width: u16, height: u16, (r, g, b): (u8, u8, u8)) -> Vec<u8> {
    let value = (((r as u16) & 0xf8) << 8) | (((g as u16) & 0xfc) << 3) | ((b as u16) >> 3);
    let mut out = Vec::with_capacity((width as usize) * (height as usize) * 2);
    for _ in 0..(width as usize * height as usize) {
        out.push((value >> 8) as u8);
        out.push((value & 0xff) as u8);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::PortIo;
    use crate::errors::{AppError, AppResult};
    use crate::protocol::{load_lcd_address_packet, set_size_packet, set_xy_packet};
    use std::collections::VecDeque;

    #[derive(Default)]
    struct MockPort {
        writes: Vec<Vec<u8>>,
        reads: VecDeque<Vec<u8>>,
        read_calls: Vec<(u64, u64)>,
        fail_on_write: Option<usize>,
    }

    impl MockPort {
        fn failing_probe() -> Self {
            Self {
                writes: Vec::new(),
                reads: VecDeque::from(vec![vec![], vec![], vec![0xde, 0xad]]),
                read_calls: Vec::new(),
                fail_on_write: None,
            }
        }

        fn fail_on_write(write_number: usize) -> Self {
            Self {
                writes: Vec::new(),
                reads: VecDeque::new(),
                read_calls: Vec::new(),
                fail_on_write: Some(write_number),
            }
        }
    }

    impl PortIo for MockPort {
        fn write_all(&mut self, bytes: &[u8]) -> AppResult<()> {
            if self.fail_on_write == Some(self.writes.len() + 1) {
                return Err(AppError::Io("forced write failure".to_string()));
            }
            self.writes.push(bytes.to_vec());
            Ok(())
        }

        fn read_idle(&mut self, total_ms: u64, idle_ms: u64) -> AppResult<Vec<u8>> {
            self.read_calls.push((total_ms, idle_ms));
            if let Some(reply) = self.reads.pop_front() {
                return Ok(reply);
            }
            let Some(last) = self.writes.last() else {
                return Ok(vec![]);
            };
            if last == &[0x02, 0x03, 0x07, 0x00, 0x00, 0x00] {
                return Ok(last.clone());
            }
            Ok(vec![])
        }
    }

    #[test]
    fn probe_failure_disables_screen_status() {
        let mut port = MockPort::failing_probe();

        let status = ScreenStatus::probe(&mut port);

        assert!(!status.is_enabled());
    }

    #[test]
    fn disabled_status_sends_no_updates() {
        let mut port = MockPort::failing_probe();
        let mut status = ScreenStatus::probe(&mut port);
        let writes_after_probe = port.writes.len();

        status.start(&mut port);
        status.update(&mut port, 25);
        status.finish(&mut port);

        assert_eq!(port.writes.len(), writes_after_probe);
    }

    #[test]
    fn start_writes_full_status_screen_first() {
        let mut port = MockPort::default();
        let mut status = ScreenStatus::probe(&mut port);
        let writes_after_probe = port.writes.len();

        status.start(&mut port);

        let start_writes = &port.writes[writes_after_probe..];
        assert_eq!(start_writes[0], vec![0x02, 0x00, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(start_writes[1], vec![0x02, 0x01, 0x00, 0xa0, 0x00, 0x50]);
        assert_eq!(start_writes[2], vec![0x02, 0x03, 0x07, 0x00, 0x00, 0x00]);
        assert_eq!(start_writes.len(), 103);
    }

    #[test]
    fn progress_updates_on_each_percent_change() {
        let mut port = MockPort::default();
        let mut status = ScreenStatus::probe(&mut port);
        status.start(&mut port);
        let writes_after_start = port.writes.len();

        status.update(&mut port, 1);
        assert!(port.writes.len() > writes_after_start);

        let writes_after_1 = port.writes.len();
        status.update(&mut port, 1);
        assert_eq!(port.writes.len(), writes_after_1);

        status.update(&mut port, 2);
        assert!(port.writes.len() > writes_after_1);
        let writes_after_2 = port.writes.len();

        status.update(&mut port, 2);
        assert_eq!(port.writes.len(), writes_after_2);
    }

    #[test]
    fn write_failure_disables_later_updates() {
        let mut port = MockPort::fail_on_write(5);
        let mut status = ScreenStatus::probe(&mut port);

        status.start(&mut port);
        assert!(!status.is_enabled());

        let writes_after_failure = port.writes.len();
        status.update(&mut port, 50);

        assert_eq!(port.writes.len(), writes_after_failure);
    }

    #[test]
    fn waiting_to_flash_writes_full_screen_once() {
        let mut port = MockPort::default();

        show_waiting_to_flash(&mut port).unwrap();

        assert_eq!(port.writes.len(), 103);
        assert_eq!(port.writes[0], set_xy_packet(0, 0).to_vec());
        assert_eq!(port.writes[1], set_size_packet(160, 80).to_vec());
        assert_eq!(port.writes[2], load_lcd_address_packet().to_vec());
        assert_eq!(port.read_calls, vec![(300, 40), (250, 40)]);
    }

    #[test]
    fn flash_done_writes_full_screen_and_settles() {
        let mut port = MockPort::default();

        show_flash_done(&mut port).unwrap();

        assert_eq!(port.writes.len(), 103);
        assert_eq!(port.writes[0], set_xy_packet(0, 0).to_vec());
        assert_eq!(port.writes[1], set_size_packet(160, 80).to_vec());
        assert_eq!(port.writes[2], load_lcd_address_packet().to_vec());
        assert_eq!(port.read_calls, vec![(300, 40), (250, 40)]);
    }

    #[test]
    fn keepalive_pixel_writes_one_black_pixel() {
        let mut port = MockPort::default();

        keepalive_pixel(&mut port).unwrap();

        assert_eq!(port.writes.len(), 4);
        assert_eq!(port.writes[0], set_xy_packet(159, 79).to_vec());
        assert_eq!(port.writes[1], set_size_packet(1, 1).to_vec());
        assert_eq!(port.writes[2], load_lcd_address_packet().to_vec());
        assert_eq!(&port.writes[3][0..6], &[0x04, 0x00, 0x00, 0x00, 0xff, 0xff]);
        assert_eq!(port.read_calls, vec![(300, 40)]);
    }
}
