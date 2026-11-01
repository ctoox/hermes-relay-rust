//! Hermes Relay - 错误处理模块

use thiserror::Error;

/// 错误类型
#[derive(Error, Debug)]
pub enum RelayError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tungstenite::Error),
    
    #[error("Config error: {0}")]
    Config(String),
    
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    
    #[error("Command error: {0}")]
    Command(String),
    
    #[error("Authentication error: {0}")]
    Auth(String),
    
    #[error("Feishu API error: {code} - {message}")]
    FeishuApi { code: i32, message: String },
    
    #[error("Timeout error: {0}")]
    Timeout(String),
    
    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Result 类型
pub type Result<T> = std::result::Result<T, RelayError>;
