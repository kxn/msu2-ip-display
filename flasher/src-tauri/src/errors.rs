use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("未连接")]
    NoDevice,
    #[error("正在写入")]
    FlashAlreadyRunning,
    #[error("{0} 被占用")]
    PortBusy(String),
    #[error("设备无响应")]
    HandshakeFailed,
    #[error("写入失败")]
    Protocol(String),
    #[error("设备已断开")]
    Disconnected,
    #[error("写入失败")]
    Asset(String),
    #[error("写入失败")]
    Io(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct UiError {
    pub message: String,
    pub detail: String,
}

impl AppError {
    pub fn user_message(&self) -> String {
        self.to_string()
    }

    pub fn detail(&self) -> String {
        match self {
            AppError::NoDevice => "No target serial device found".to_string(),
            AppError::FlashAlreadyRunning => "Flash job is already running".to_string(),
            AppError::PortBusy(port) => format!("Failed to open serial port {port}"),
            AppError::HandshakeFailed => {
                "Handshake did not return expected MSNCN bytes".to_string()
            }
            AppError::Protocol(detail) => detail.clone(),
            AppError::Disconnected => "Serial read/write failed during flashing".to_string(),
            AppError::Asset(detail) => detail.clone(),
            AppError::Io(detail) => detail.clone(),
        }
    }

    pub fn to_ui_error(&self) -> UiError {
        UiError {
            message: self.user_message(),
            detail: self.detail(),
        }
    }
}
