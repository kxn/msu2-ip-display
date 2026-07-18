use thiserror::Error;

pub const IMAGE_BYTES: usize = 25_600;
pub const OFFLINE_FRAME_COUNT: usize = 36;
pub const OFFLINE_ANIMATION_BYTES: usize = IMAGE_BYTES * OFFLINE_FRAME_COUNT;
pub const PAGES_PER_IMAGE: u16 = 100;
pub const DHCP_FAILED_PAGE: u16 = 3726;
pub const ACQUIRING_PAGE: u16 = 3826;
pub const IP_BG_PAGE: u16 = 3926;
pub const PRESERVED_FONT_START_PAGE: u16 = 4026;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AssetError {
    #[error("{label} has {actual} bytes, expected {expected}")]
    WrongSize {
        label: &'static str,
        actual: usize,
        expected: usize,
    },
    #[error(
        "{label} would end at page {end_page}, which reaches preserved page {preserved_start}"
    )]
    LayoutOverlapsPreservedPages {
        label: &'static str,
        end_page: u16,
        preserved_start: u16,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct FlashImage<'a> {
    pub label: &'static str,
    pub start_page: u16,
    pub bytes: &'a [u8],
}

impl<'a> FlashImage<'a> {
    pub fn end_page(&self) -> u16 {
        self.start_page + PAGES_PER_IMAGE - 1
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EmbeddedAssets {
    pub offline_animation: &'static [u8],
    pub acquiring: &'static [u8],
    pub dhcp_failed: &'static [u8],
    pub ip_bg: &'static [u8],
}

impl EmbeddedAssets {
    pub fn offline_frame(&self, frame: usize) -> &'static [u8] {
        let start = frame * IMAGE_BYTES;
        let end = start + IMAGE_BYTES;
        &self.offline_animation[start..end]
    }
}

pub fn embedded_assets() -> EmbeddedAssets {
    EmbeddedAssets {
        offline_animation: include_bytes!("../assets/offline_animation.rgb565be"),
        acquiring: include_bytes!("../assets/acquiring.rgb565be"),
        dhcp_failed: include_bytes!("../assets/dhcp_failed.rgb565be"),
        ip_bg: include_bytes!("../assets/ip_bg.rgb565be"),
    }
}

pub fn validate_image(label: &'static str, bytes: &[u8]) -> Result<(), AssetError> {
    if bytes.len() != IMAGE_BYTES {
        return Err(AssetError::WrongSize {
            label,
            actual: bytes.len(),
            expected: IMAGE_BYTES,
        });
    }

    Ok(())
}

pub fn fixed_flash_plan<'a>(assets: &'a EmbeddedAssets) -> Vec<FlashImage<'a>> {
    let mut plan = Vec::with_capacity(OFFLINE_FRAME_COUNT + 3);

    for frame in 0..OFFLINE_FRAME_COUNT {
        plan.push(FlashImage {
            label: "offline",
            start_page: (frame as u16) * PAGES_PER_IMAGE,
            bytes: assets.offline_frame(frame),
        });
    }

    plan.push(FlashImage {
        label: "dhcp_failed",
        start_page: DHCP_FAILED_PAGE,
        bytes: assets.dhcp_failed,
    });

    plan.push(FlashImage {
        label: "acquiring",
        start_page: ACQUIRING_PAGE,
        bytes: assets.acquiring,
    });

    plan.push(FlashImage {
        label: "ip_bg",
        start_page: IP_BG_PAGE,
        bytes: assets.ip_bg,
    });

    plan
}

pub fn validate_plan(plan: &[FlashImage<'_>]) -> Result<(), AssetError> {
    for item in plan {
        validate_image(item.label, item.bytes)?;
        if item.end_page() >= PRESERVED_FONT_START_PAGE {
            return Err(AssetError::LayoutOverlapsPreservedPages {
                label: item.label,
                end_page: item.end_page(),
                preserved_start: PRESERVED_FONT_START_PAGE,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_assets_have_verified_size() {
        let assets = embedded_assets();
        assert_eq!(assets.offline_animation.len(), OFFLINE_ANIMATION_BYTES);
        assert_eq!(assets.acquiring.len(), IMAGE_BYTES);
        assert_eq!(assets.dhcp_failed.len(), IMAGE_BYTES);
        assert_eq!(assets.ip_bg.len(), IMAGE_BYTES);
    }

    #[test]
    fn fixed_plan_has_39_images_and_preserves_font_pages() {
        let assets = embedded_assets();
        let plan = fixed_flash_plan(&assets);
        assert_eq!(plan.len(), OFFLINE_FRAME_COUNT + 3);
        assert_eq!(plan[0].start_page, 0);
        assert_eq!(plan[35].start_page, 3500);
        assert_eq!(plan[36].start_page, DHCP_FAILED_PAGE);
        assert_eq!(plan[37].start_page, ACQUIRING_PAGE);
        assert_eq!(plan[38].start_page, IP_BG_PAGE);

        for item in plan {
            assert_eq!(item.bytes.len(), IMAGE_BYTES);
            assert!(item.end_page() < PRESERVED_FONT_START_PAGE);
        }
    }

    #[test]
    fn fixed_plan_contains_dhcp_failed_status_page() {
        let assets = embedded_assets();
        let plan = fixed_flash_plan(&assets);
        let item = plan
            .iter()
            .find(|item| item.label == "dhcp_failed")
            .expect("dhcp_failed status asset should be flashed");

        assert_eq!(item.start_page, DHCP_FAILED_PAGE);
        assert_eq!(item.end_page(), 3825);
        assert_eq!(item.bytes.len(), IMAGE_BYTES);
    }

    #[test]
    fn fixed_plan_uses_distinct_offline_animation_frames() {
        let assets = embedded_assets();
        let plan = fixed_flash_plan(&assets);
        let offline_frames = &plan[..36];
        let first_frame = offline_frames[0].bytes;

        assert!(
            offline_frames.iter().any(|item| item.bytes != first_frame),
            "offline animation should not flash the same image into every frame"
        );
    }

    #[test]
    fn fixed_plan_uses_hard_cut_offline_blink_frames() {
        const VISIBLE_OFFLINE: &[u8] = include_bytes!("../assets/offline.rgb565be");

        let assets = embedded_assets();
        let plan = fixed_flash_plan(&assets);
        let offline_frames = &plan[..36];

        assert!(
            offline_frames
                .iter()
                .any(|item| item.bytes == VISIBLE_OFFLINE),
            "offline animation should include fully visible 未连接 frames"
        );
        assert!(
            offline_frames.iter().any(|item| item.bytes == assets.ip_bg),
            "offline animation should include fully blank background frames"
        );
    }

    #[test]
    fn wrong_size_is_rejected() {
        let err = validate_image("bad", &[0u8; 12]).unwrap_err();
        assert_eq!(err.to_string(), "bad has 12 bytes, expected 25600");
    }
}
