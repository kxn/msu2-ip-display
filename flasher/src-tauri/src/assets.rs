use thiserror::Error;

pub const PAGE_BYTES: usize = 256;
pub const RGB_IMAGE_BYTES: usize = 25_600;
pub const IMAGE_BYTES: usize = RGB_IMAGE_BYTES;
pub const OFFLINE_FRAME_COUNT: usize = 36;
pub const OFFLINE_ANIMATION_BYTES: usize = IMAGE_BYTES * OFFLINE_FRAME_COUNT;
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

pub fn validate_image(label: &'static str, bytes: &[u8]) -> Result<(), AssetError> {
    validate_asset(&FlashAsset {
        label,
        start_page: 0,
        page_count: RGB_IMAGE_PAGES,
        bytes,
    })
}

pub fn fixed_flash_plan<'a>(assets: &'a EmbeddedAssets) -> Vec<FlashImage<'a>> {
    let mut plan = Vec::with_capacity(OFFLINE_FRAME_COUNT + 3);

    for frame in 0..OFFLINE_FRAME_COUNT {
        plan.push(FlashImage {
            label: "offline",
            start_page: (frame as u16) * PAGES_PER_IMAGE,
            page_count: RGB_IMAGE_PAGES,
            bytes: assets.offline_frame(frame),
        });
    }

    plan.push(FlashImage {
        label: "dhcp_failed",
        start_page: DHCP_FAILED_PAGE,
        page_count: RGB_IMAGE_PAGES,
        bytes: assets.dhcp_failed,
    });

    plan.push(FlashImage {
        label: "acquiring",
        start_page: ACQUIRING_PAGE,
        page_count: RGB_IMAGE_PAGES,
        bytes: assets.acquiring,
    });

    plan.push(FlashImage {
        label: "ip_bg",
        start_page: IP_BG_PAGE,
        page_count: RGB_IMAGE_PAGES,
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
