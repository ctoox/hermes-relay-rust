//! Hermes Relay - 协议模块
//! 定义消息类型和通信协议

use serde::{Deserialize, Serialize};
use chrono::Utc;
use uuid::Uuid;
use std::collections::HashMap;

/// 消息类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    // 命令相关
    Command,
    Response,
    
    // 设备管理
    Register,
    RegisterAck,
    Heartbeat,
    Pong,
    DeviceList,
    DeviceStatus,
    
    // 审批相关
    ApprovalRequest,
    ApprovalResponse,
    
    // 控制
    Interrupt,
    Cancel,
    
    // 文件传输
    FileUpload,
    FileDownload,
    FileData,
    
    // 系统
    Error,
    Ack,
    Ping,
}

/// 基础消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: MessageType,
    pub timestamp: f64,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub target: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl ProtocolMessage {
    /// 创建新消息
    pub fn new(msg_type: MessageType, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            msg_type,
            timestamp: Utc::now().timestamp_millis() as f64 / 1000.0,
            source: String::new(),
            target: String::new(),
            version: "1.0".to_string(),
            payload,
        }
    }
    
    /// 转换为 JSON 字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    
    /// 从 JSON 字符串解析
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// 基础消息 (别名)
pub type Message = ProtocolMessage;

/// 命令消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandPayload {
    pub command_id: String,
    pub command: String,
    #[serde(default)]
    pub working_dir: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
}

fn default_timeout() -> u64 {
    30
}

impl CommandPayload {
    pub fn new(command: &str) -> Self {
        Self {
            command_id: Uuid::new_v4().to_string(),
            command: command.to_string(),
            working_dir: String::new(),
            timeout: 30,
            env_vars: HashMap::new(),
        }
    }
}

/// 响应消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsePayload {
    pub command_id: String,
    pub status: CommandStatus,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub error_output: String,
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub execution_time: f64,
}

/// 命令状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    Success,
    Error,
    Timeout,
    Cancelled,
}

impl std::fmt::Display for CommandStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::Error => write!(f, "error"),
            Self::Timeout => write!(f, "timeout"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// 心跳消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatPayload {
    pub device_id: String,
    #[serde(default)]
    pub device_name: String,
    #[serde(default = "default_group")]
    pub device_group: String,
    #[serde(default)]
    pub cpu_usage: f64,
    #[serde(default)]
    pub memory_usage: f64,
    #[serde(default)]
    pub disk_usage: f64,
    #[serde(default)]
    pub uptime: f64,
    #[serde(default)]
    pub active_commands: u32,
}

fn default_group() -> String {
    "default".to_string()
}

/// 注册消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterPayload {
    pub device_id: String,
    pub device_name: String,
    #[serde(default = "default_group")]
    pub device_group: String,
    #[serde(default)]
    pub os_version: String,
    #[serde(default)]
    pub ip_address: String,
    #[serde(default)]
    pub mac_address: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub auth_token: String,
}

/// 注册确认
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAckPayload {
    pub device_id: String,
    pub status: String,
    pub server_time: f64,
}

/// 审批请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequestPayload {
    pub approval_id: String,
    pub command: String,
    pub command_preview: String,
    pub device_id: String,
    #[serde(default)]
    pub device_name: String,
    #[serde(default = "default_risk")]
    pub risk_level: RiskLevel,
    #[serde(default)]
    pub reason: String,
    #[serde(default = "default_approval_timeout")]
    pub timeout: u64,
}

fn default_risk() -> RiskLevel {
    RiskLevel::Medium
}

fn default_approval_timeout() -> u64 {
    300
}

/// 风险等级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
    Blocked,
}

/// 审批响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResponsePayload {
    pub approval_id: String,
    pub action: ApprovalAction,
    #[serde(default)]
    pub approver: String,
}

/// 审批动作
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalAction {
    AllowOnce,
    AllowSession,
    AllowAlways,
    Deny,
}

/// 错误消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub error_code: String,
    pub error_message: String,
    #[serde(default)]
    pub details: Option<serde_json::Value>,
}

/// 取消命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelPayload {
    pub command_id: String,
}

/// 文件传输
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDataPayload {
    pub file_name: String,
    pub file_size: u64,
    pub file_hash: String,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub data: String,  // Base64 encoded
    pub is_last_chunk: bool,
}
