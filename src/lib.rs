//! Hermes Relay - 高性能远程控制系统
//!
//! 基于飞书的远程电脑控制系统，支持自然语言操作、命令审批、多设备群控。

pub mod error;
pub mod protocol;
pub mod config;
pub mod server;
pub mod client;
pub mod device;
pub mod command;
pub mod nlp;
pub mod feishu;

pub use error::{RelayError, Result};
pub use protocol::{ProtocolMessage, MessageType};
pub use config::{RelayConfig, AgentConfig};
