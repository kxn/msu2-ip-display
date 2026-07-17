use std::path::PathBuf;

#[cfg(target_os = "linux")]
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtyDevice {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbAttrs {
    pub id_vendor: String,
    pub id_product: String,
}

pub fn match_target_tty(_name: &str, _ancestry: &[UsbAttrs]) -> bool {
    _ancestry.iter().any(|attrs| {
        attrs.id_vendor.eq_ignore_ascii_case("1a86")
            && attrs.id_product.eq_ignore_ascii_case("fe0c")
    })
}

#[cfg(target_os = "linux")]
pub fn scan_target_ttys() -> std::io::Result<Vec<TtyDevice>> {
    scan_target_ttys_from(Path::new("/sys/class/tty"), Path::new("/dev"))
}

#[cfg(target_os = "linux")]
pub fn scan_target_ttys_from(
    sys_class_tty: &Path,
    dev_root: &Path,
) -> std::io::Result<Vec<TtyDevice>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(sys_class_tty)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let device_path = entry.path().join("device");
        let mut attrs = Vec::new();
        collect_usb_attrs(&device_path, &mut attrs);
        if match_target_tty(&name, &attrs) {
            out.push(TtyDevice {
                path: dev_root.join(&name),
                name,
            });
        }
    }
    Ok(out)
}

#[cfg(target_os = "linux")]
fn collect_usb_attrs(path: &Path, attrs: &mut Vec<UsbAttrs>) {
    let mut current = path.to_path_buf();
    for _ in 0..8 {
        let vendor = std::fs::read_to_string(current.join("idVendor"));
        let product = std::fs::read_to_string(current.join("idProduct"));
        if let (Ok(vendor), Ok(product)) = (vendor, product) {
            attrs.push(UsbAttrs {
                id_vendor: vendor.trim().to_string(),
                id_product: product.trim().to_string(),
            });
        }
        if !current.pop() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_verified_vid_pid_in_usb_ancestry() {
        assert!(match_target_tty(
            "ttyACM0",
            &[UsbAttrs {
                id_vendor: "1a86".to_string(),
                id_product: "fe0c".to_string(),
            }]
        ));
    }

    #[test]
    fn rejects_wrong_vid_pid() {
        assert!(!match_target_tty(
            "ttyUSB0",
            &[UsbAttrs {
                id_vendor: "1a86".to_string(),
                id_product: "7523".to_string(),
            }]
        ));
    }

    #[test]
    fn accepts_uppercase_sysfs_hex() {
        assert!(match_target_tty(
            "ttyACM0",
            &[UsbAttrs {
                id_vendor: "1A86".to_string(),
                id_product: "FE0C".to_string(),
            }]
        ));
    }
}
