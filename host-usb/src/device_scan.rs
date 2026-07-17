#[cfg(any(target_os = "linux", test))]
use std::path::Path;
use std::path::PathBuf;

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
        collect_usb_attrs_from_device_path(&device_path, &mut attrs, |path| {
            std::fs::canonicalize(path)
        });
        if match_target_tty(&name, &attrs) {
            out.push(TtyDevice {
                path: dev_root.join(&name),
                name,
            });
        }
    }
    Ok(out)
}

#[cfg(any(target_os = "linux", test))]
fn collect_usb_attrs_from_device_path<F>(path: &Path, attrs: &mut Vec<UsbAttrs>, resolve: F)
where
    F: FnOnce(&Path) -> std::io::Result<PathBuf>,
{
    let resolved = resolve(path).unwrap_or_else(|_| path.to_path_buf());
    collect_usb_attrs_from_resolved_path(&resolved, attrs);
}

#[cfg(any(target_os = "linux", test))]
fn collect_usb_attrs_from_resolved_path(path: &Path, attrs: &mut Vec<UsbAttrs>) {
    let mut current = path.to_path_buf();
    for _ in 0..16 {
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

    #[test]
    fn collects_vid_pid_from_ancestor_of_resolved_device_symlink_target() {
        let root = unique_temp_dir("miniboard-device-scan");
        let usb_device = root.join("sys/devices/pci0000_00/usb1/1-1");
        let tty_leaf = usb_device.join("1-1_1.0/tty/ttyACM0");
        std::fs::create_dir_all(&tty_leaf).unwrap();
        std::fs::write(usb_device.join("idVendor"), "1A86\n").unwrap();
        std::fs::write(usb_device.join("idProduct"), "FE0C\n").unwrap();

        let textual_device_path = root.join("sys/class/tty/ttyACM0/device");
        let mut attrs = Vec::new();
        collect_usb_attrs_from_device_path(&textual_device_path, &mut attrs, |_| {
            Ok(tty_leaf.clone())
        });

        assert_eq!(
            attrs,
            vec![UsbAttrs {
                id_vendor: "1A86".to_string(),
                id_product: "FE0C".to_string(),
            }]
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
