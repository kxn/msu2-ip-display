use crate::device::PortIo;
use crate::flasher::{write_lcd_region, RetryPolicy};

const GREEN: (u8, u8, u8) = (51, 255, 127);
const PROGRESS_INNER_X: u16 = 26;
const PROGRESS_INNER_Y: u16 = 62;
const PROGRESS_INNER_WIDTH: u16 = 108;
const PROGRESS_INNER_HEIGHT: u16 = 4;

struct RegionAsset {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    bytes: &'static [u8],
}

const TITLE: RegionAsset = RegionAsset {
    x: 33,
    y: 16,
    width: 94,
    height: 28,
    bytes: include_bytes!("../assets/flash_status_title.rgb565be"),
};

const PROGRESS_FRAME: RegionAsset = RegionAsset {
    x: 24,
    y: 60,
    width: 112,
    height: 8,
    bytes: include_bytes!("../assets/flash_status_progress_frame.rgb565be"),
};

const PERCENT_000: RegionAsset = RegionAsset {
    x: 62,
    y: 42,
    width: 36,
    height: 13,
    bytes: include_bytes!("../assets/flash_status_percent_000.rgb565be"),
};

const PERCENT_025: RegionAsset = RegionAsset {
    x: 62,
    y: 42,
    width: 36,
    height: 13,
    bytes: include_bytes!("../assets/flash_status_percent_025.rgb565be"),
};

const PERCENT_050: RegionAsset = RegionAsset {
    x: 62,
    y: 42,
    width: 36,
    height: 13,
    bytes: include_bytes!("../assets/flash_status_percent_050.rgb565be"),
};

const PERCENT_075: RegionAsset = RegionAsset {
    x: 62,
    y: 42,
    width: 36,
    height: 13,
    bytes: include_bytes!("../assets/flash_status_percent_075.rgb565be"),
};

const PERCENT_100: RegionAsset = RegionAsset {
    x: 62,
    y: 42,
    width: 36,
    height: 13,
    bytes: include_bytes!("../assets/flash_status_percent_100.rgb565be"),
};

const DONE: RegionAsset = RegionAsset {
    x: 34,
    y: 16,
    width: 94,
    height: 44,
    bytes: include_bytes!("../assets/flash_status_done.rgb565be"),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenStatus {
    enabled: bool,
    last_bucket: Option<u8>,
    last_fill_px: u16,
}

impl ScreenStatus {
    pub fn probe<P: PortIo>(port: &mut P) -> Self {
        let mut status = Self {
            enabled: true,
            last_bucket: None,
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
        self.write_asset(port, &TITLE);
        self.write_asset(port, &PERCENT_000);
        self.write_asset(port, &PROGRESS_FRAME);
        if self.enabled {
            self.last_bucket = Some(0);
            self.last_fill_px = 0;
        }
    }

    pub fn update<P: PortIo>(&mut self, port: &mut P, percent: u8) {
        if !self.enabled {
            return;
        }

        let bucket = percent_bucket(percent);
        if self.last_bucket == Some(bucket) {
            return;
        }

        self.write_asset(port, percent_asset(bucket));
        if !self.enabled {
            return;
        }

        let next_fill_px = fill_pixels(bucket);
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

        self.last_bucket = Some(bucket);
    }

    pub fn finish<P: PortIo>(&mut self, port: &mut P) {
        self.write_asset(port, &DONE);
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

fn percent_bucket(percent: u8) -> u8 {
    match percent {
        100..=u8::MAX => 100,
        75..=99 => 75,
        50..=74 => 50,
        25..=49 => 25,
        _ => 0,
    }
}

fn percent_asset(bucket: u8) -> &'static RegionAsset {
    match bucket {
        100 => &PERCENT_100,
        75 => &PERCENT_075,
        50 => &PERCENT_050,
        25 => &PERCENT_025,
        _ => &PERCENT_000,
    }
}

fn fill_pixels(bucket: u8) -> u16 {
    ((PROGRESS_INNER_WIDTH as u32 * bucket as u32) / 100) as u16
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
    use std::collections::VecDeque;

    #[derive(Default)]
    struct MockPort {
        writes: Vec<Vec<u8>>,
        reads: VecDeque<Vec<u8>>,
        fail_on_write: Option<usize>,
    }

    impl MockPort {
        fn failing_probe() -> Self {
            Self {
                writes: Vec::new(),
                reads: VecDeque::from(vec![vec![], vec![], vec![0xde, 0xad]]),
                fail_on_write: None,
            }
        }

        fn fail_on_write(write_number: usize) -> Self {
            Self {
                writes: Vec::new(),
                reads: VecDeque::new(),
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

        fn read_idle(&mut self, _total_ms: u64, _idle_ms: u64) -> AppResult<Vec<u8>> {
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
    fn progress_updates_only_when_bucket_changes() {
        let mut port = MockPort::default();
        let mut status = ScreenStatus::probe(&mut port);
        status.start(&mut port);
        let writes_after_start = port.writes.len();

        status.update(&mut port, 10);
        assert_eq!(port.writes.len(), writes_after_start);

        status.update(&mut port, 25);
        assert!(port.writes.len() > writes_after_start);
        let writes_after_25 = port.writes.len();

        status.update(&mut port, 37);
        assert_eq!(port.writes.len(), writes_after_25);

        status.update(&mut port, 50);
        assert!(port.writes.len() > writes_after_25);
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
}
