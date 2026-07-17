use std::io::{Read, Write};
use std::time::{Duration, Instant};

use serde::Serialize;
use serialport::{
    DataBits, FlowControl, Parity, SerialPort, SerialPortInfo, SerialPortType, StopBits,
};

use crate::errors::{AppError, AppResult};
use crate::protocol::{contains_sequence, HANDSHAKE};

const TARGET_VID: u16 = 0x1a86;
const TARGET_PID: u16 = 0xfe0c;
const BAUD_RATE: u32 = 19_200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SerialPortSettings {
    baud_rate: u32,
    data_bits: DataBits,
    parity: Parity,
    stop_bits: StopBits,
    flow_control: FlowControl,
}

fn serial_port_settings() -> SerialPortSettings {
    SerialPortSettings {
        baud_rate: BAUD_RATE,
        data_bits: DataBits::Eight,
        parity: Parity::None,
        stop_bits: StopBits::One,
        flow_control: FlowControl::None,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DeviceInfo {
    pub port_name: String,
    pub vid_pid: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
}

pub trait PortIo {
    fn write_all(&mut self, bytes: &[u8]) -> AppResult<()>;
    fn read_idle(&mut self, total_ms: u64, idle_ms: u64) -> AppResult<Vec<u8>>;
}

pub struct SerialPortIo {
    inner: Box<dyn SerialPort>,
}

impl SerialPortIo {
    pub fn new(inner: Box<dyn SerialPort>) -> Self {
        Self { inner }
    }
}

impl PortIo for SerialPortIo {
    fn write_all(&mut self, bytes: &[u8]) -> AppResult<()> {
        self.inner
            .write_all(bytes)
            .map_err(|err| AppError::Io(err.to_string()))?;
        self.inner
            .flush()
            .map_err(|err| AppError::Io(err.to_string()))?;
        Ok(())
    }

    fn read_idle(&mut self, total_ms: u64, idle_ms: u64) -> AppResult<Vec<u8>> {
        let deadline = Instant::now() + Duration::from_millis(total_ms);
        let mut idle_deadline = Instant::now() + Duration::from_millis(idle_ms);
        let mut out = Vec::new();
        let mut buf = [0u8; 256];

        while Instant::now() < deadline && Instant::now() < idle_deadline {
            match self.inner.read(&mut buf) {
                Ok(0) => std::thread::sleep(Duration::from_millis(3)),
                Ok(n) => {
                    out.extend_from_slice(&buf[..n]);
                    idle_deadline = Instant::now() + Duration::from_millis(idle_ms);
                }
                Err(err) if err.kind() == std::io::ErrorKind::TimedOut => {
                    std::thread::sleep(Duration::from_millis(3));
                }
                Err(err) => return Err(AppError::Io(err.to_string())),
            }
        }

        Ok(out)
    }
}

pub fn matches_target_usb(vid: Option<u16>, pid: Option<u16>, text: Option<&str>) -> bool {
    if vid == Some(TARGET_VID) && pid == Some(TARGET_PID) {
        return true;
    }

    let Some(text) = text else {
        return false;
    };

    let lower = text.to_ascii_lowercase();
    lower.contains("ch32") || lower.contains("ch32x035")
}

pub fn info_from_port(port: &SerialPortInfo) -> Option<DeviceInfo> {
    match &port.port_type {
        SerialPortType::UsbPort(usb) => {
            let text = usb
                .product
                .as_deref()
                .or(usb.manufacturer.as_deref())
                .or(usb.serial_number.as_deref());
            if !matches_target_usb(Some(usb.vid), Some(usb.pid), text) {
                return None;
            }

            Some(DeviceInfo {
                port_name: port.port_name.clone(),
                vid_pid: Some(format!("{:04X}:{:04X}", usb.vid, usb.pid)),
                product: usb.product.clone(),
                serial: usb.serial_number.clone(),
            })
        }
        other => {
            let text = format!("{other:?}");
            if !matches_target_usb(None, None, Some(&text)) {
                return None;
            }

            Some(DeviceInfo {
                port_name: port.port_name.clone(),
                vid_pid: None,
                product: Some(text),
                serial: None,
            })
        }
    }
}

pub fn scan_candidates() -> AppResult<Vec<DeviceInfo>> {
    let ports = serialport::available_ports().map_err(|err| AppError::Io(err.to_string()))?;
    Ok(ports.iter().filter_map(info_from_port).collect())
}

pub fn open_serial_port(port_name: &str) -> AppResult<SerialPortIo> {
    let settings = serial_port_settings();
    let port = serialport::new(port_name, settings.baud_rate)
        .data_bits(settings.data_bits)
        .parity(settings.parity)
        .stop_bits(settings.stop_bits)
        .flow_control(settings.flow_control)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|_| AppError::PortBusy(port_name.to_string()))?;
    Ok(SerialPortIo::new(port))
}

pub fn handshake<P: PortIo>(port: &mut P) -> AppResult<()> {
    port.read_idle(200, 40)?;
    port.write_all(&HANDSHAKE)?;
    let reply = port.read_idle(300, 40)?;
    if contains_sequence(&reply, &HANDSHAKE) {
        Ok(())
    } else {
        Err(AppError::HandshakeFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::{DataBits, FlowControl, Parity, StopBits};

    #[derive(Default)]
    struct MockPort {
        writes: Vec<Vec<u8>>,
        reads: Vec<Vec<u8>>,
    }

    impl PortIo for MockPort {
        fn write_all(&mut self, bytes: &[u8]) -> AppResult<()> {
            self.writes.push(bytes.to_vec());
            Ok(())
        }

        fn read_idle(&mut self, _total_ms: u64, _idle_ms: u64) -> AppResult<Vec<u8>> {
            if self.writes.is_empty() || self.reads.is_empty() {
                Ok(Vec::new())
            } else {
                Ok(self.reads.remove(0))
            }
        }
    }

    #[test]
    fn candidate_matching_accepts_verified_vid_pid() {
        assert!(matches_target_usb(Some(0x1a86), Some(0xfe0c), Some("CH32x035")));
    }

    #[test]
    fn candidate_matching_accepts_wch_text_when_vid_pid_absent() {
        assert!(matches_target_usb(None, None, Some("WCH CH32x035")));
    }

    #[test]
    fn task4_candidate_matching_rejects_vendor_only_wch_text() {
        assert!(!matches_target_usb(None, None, Some("WCH USB-SERIAL")));
    }

    #[test]
    fn candidate_matching_rejects_unrelated_port() {
        assert!(!matches_target_usb(Some(0x1234), Some(0xabcd), Some("Other")));
    }

    #[test]
    fn task4_serial_port_settings_enforce_19200_8n1() {
        let settings = serial_port_settings();

        assert_eq!(settings.baud_rate, 19_200);
        assert_eq!(settings.data_bits, DataBits::Eight);
        assert_eq!(settings.parity, Parity::None);
        assert_eq!(settings.stop_bits, StopBits::One);
        assert_eq!(settings.flow_control, FlowControl::None);
    }

    #[test]
    fn handshake_succeeds_with_verified_reply() {
        let mut port = MockPort {
            writes: vec![],
            reads: vec![crate::protocol::HANDSHAKE.to_vec()],
        };
        handshake(&mut port).unwrap();
        assert_eq!(port.writes, vec![crate::protocol::HANDSHAKE.to_vec()]);
    }

    #[test]
    fn handshake_fails_with_wrong_reply() {
        let mut port = MockPort {
            writes: vec![],
            reads: vec![vec![0x00, 0x11]],
        };
        assert!(matches!(
            handshake(&mut port),
            Err(AppError::HandshakeFailed)
        ));
    }
}
