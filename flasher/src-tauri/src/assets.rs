use thiserror::Error;

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

const PROTECTED_RANGES: &[(u16, u16)] = &[
    (3651, 3778),
    (DIGIT_RESOURCE_PAGE, 4037),
    (4038, 4044),
    (PANEL_CONFIG_PAGE, PANEL_CONFIG_PAGE),
];

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

pub fn validate_plan(plan: &[FlashAsset<'_>]) -> Result<(), AssetError> {
    for item in plan {
        validate_asset(item)?;
        for (start, end) in PROTECTED_RANGES {
            if item.start_page <= *end && item.end_page() >= *start {
                return Err(AssetError::LayoutOverlapsPreservedPages {
                    label: item.label,
                    end_page: item.end_page(),
                    preserved_start: *start,
                });
            }
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
        assert_eq!(assets.offline.len(), RGB_IMAGE_BYTES);
        assert_eq!(assets.acquiring.len(), RGB_IMAGE_BYTES);
        assert_eq!(assets.dhcp_failed.len(), RGB_IMAGE_BYTES);
        assert_eq!(assets.ip_bg.len(), RGB_IMAGE_BYTES);
        assert_eq!(
            assets.startup_logo.len(),
            PAGE_BYTES * MONO_LOGO_PAGES as usize
        );
        assert_eq!(assets.resource_directory.len(), PAGE_BYTES);
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
            assert!(
                !(asset.start_page <= 3778 && asset.end_page() >= 3651),
                "{:?}",
                asset
            );
            assert!(
                !(asset.start_page <= 4037 && asset.end_page() >= DIGIT_RESOURCE_PAGE),
                "{:?}",
                asset
            );
            assert!(asset.end_page() < PANEL_CONFIG_PAGE, "{:?}", asset);
        }
    }

    #[test]
    fn resource_directory_points_offline_mode_to_two_frames() {
        let assets = embedded_assets();
        let bytes = assets.resource_directory;
        assert_eq!(&bytes[0x04..0x08], &[0x03, 0x84, 0x00, 0x02]);
        assert_eq!(&bytes[0x08..0x0b], &[0x00, 0x00, 0x00]);
        assert_eq!(&bytes[0x20 + 0x04..0x20 + 0x08], &[0x00, 0x64, 0x00, 0x01]);
        assert_eq!(&bytes[0x20 + 0x08..0x20 + 0x0b], &[0x00, 0x00, 0xC8]);
        assert_eq!(&bytes[0x40 + 0x08..0x40 + 0x0b], &[0x00, 0x0E, 0xEC]);
    }

    #[test]
    fn wrong_size_is_rejected() {
        let err = validate_image("bad", &[0u8; 12]).unwrap_err();
        assert_eq!(err.to_string(), "bad has 12 bytes, expected 25600");
    }
}
