//! Hermes Relay - 配置模块

use serde::{Deserialize, Serialize};
use std::path::Path;

/// 中转站配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// WebSocket 监听地址
    #[serde(default = "default_ws_host")]
    pub ws_host: String,
    
    /// WebSocket 监听端口
    #[serde(default = "default_ws_port")]
    pub ws_port: u16,
    
    /// 飞书配置
    #[serde(default)]
    pub feishu: FeishuConfig,
    
    /// 心跳超时时间(秒)
    #[serde(default = "default_heartbeat_timeout")]
    pub heartbeat_timeout: u64,
    
    /// 审批超时时间(秒)
    #[serde(default = "default_approval_timeout")]
    pub approval_timeout: u64,
    
    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,
    
    /// 认证令牌
    #[serde(default)]
    pub auth_token: String,
    
    /// 是否启用加密
    #[serde(default)]
    pub enable_encryption: bool,
}

fn default_ws_host() -> String { "0.0.0.0".to_string() }
fn default_ws_port() -> u16 { 8765 }
fn default_heartbeat_timeout() -> u64 { 60 }
fn default_approval_timeout() -> u64 { 300 }
fn default_log_level() -> String { "info".to_string() }

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            ws_host: default_ws_host(),
            ws_port: default_ws_port(),
            feishu: FeishuConfig::default(),
            heartbeat_timeout: default_heartbeat_timeout(),
            approval_timeout: default_approval_timeout(),
            log_level: default_log_level(),
            auth_token: String::new(),
            enable_encryption: false,
        }
    }
}

/// 飞书配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuConfig {
    /// 应用 ID
    #[serde(default)]
    pub app_id: String,
    
    /// 应用密钥
    #[serde(default)]
    pub app_secret: String,
    
    /// 域名 (feishu/lark)
    #[serde(default = "default_domain")]
    pub domain: String,
    
    /// 加密密钥
    #[serde(default)]
    pub encrypt_key: String,
    
    /// 验证令牌
    #[serde(default)]
    pub verification_token: String,
}

fn default_domain() -> String { "feishu".to_string() }

impl Default for FeishuConfig {
    fn default() -> Self {
        Self {
            app_id: String::new(),
            app_secret: String::new(),
            domain: default_domain(),
            encrypt_key: String::new(),
            verification_token: String::new(),
        }
    }
}

/// 被控端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// 设备ID
    #[serde(default = "generate_uuid")]
    pub device_id: String,
    
    /// 设备名称
    #[serde(default = "get_hostname")]
    pub device_name: String,
    
    /// 设备组
    #[serde(default = "default_group")]
    pub device_group: String,
    
    /// 服务端地址
    #[serde(default = "default_server_url")]
    pub server_url: String,
    
    /// 认证令牌
    #[serde(default)]
    pub auth_token: String,
    
    /// 心跳间隔(秒)
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval: u64,
    
    /// 命令超时时间(秒)
    #[serde(default = "default_command_timeout")]
    pub command_timeout: u64,
    
    /// 最大并发命令数
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_commands: u32,
    
    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,
    
    /// 重连间隔(秒)
    #[serde(default = "default_reconnect_interval")]
    pub reconnect_interval: u64,
    
    /// 最大重连次数 (0=无限)
    #[serde(default)]
    pub max_reconnect_attempts: u32,
}

fn generate_uuid() -> String { uuid::Uuid::new_v4().to_string() }
fn get_hostname() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}
fn default_group() -> String { "default".to_string() }
fn default_server_url() -> String { "ws://localhost:8765".to_string() }
fn default_heartbeat_interval() -> u64 { 30 }
fn default_command_timeout() -> u64 { 60 }
fn default_max_concurrent() -> u32 { 5 }
fn default_reconnect_interval() -> u64 { 5 }

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            device_id: generate_uuid(),
            device_name: get_hostname(),
            device_group: default_group(),
            server_url: default_server_url(),
            auth_token: String::new(),
            heartbeat_interval: default_heartbeat_interval(),
            command_timeout: default_command_timeout(),
            max_concurrent_commands: default_max_concurrent(),
            log_level: default_log_level(),
            reconnect_interval: default_reconnect_interval(),
            max_reconnect_attempts: 0,
        }
    }
}

impl RelayConfig {
    /// 从文件加载配置
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// 从环境变量加载
    pub fn from_env() -> Self {
        let mut config = Self::default();
        
        if let Ok(host) = std::env::var("RELAY_WS_HOST") {
            config.ws_host = host;
        }
        if let Ok(port) = std::env::var("RELAY_WS_PORT") {
            if let Ok(port) = port.parse() {
                config.ws_port = port;
            }
        }
        if let Ok(app_id) = std::env::var("FEISHU_APP_ID") {
            config.feishu.app_id = app_id;
        }
        if let Ok(app_secret) = std::env::var("FEISHU_APP_SECRET") {
            config.feishu.app_secret = app_secret;
        }
        if let Ok(domain) = std::env::var("FEISHU_DOMAIN") {
            config.feishu.domain = domain;
        }
        if let Ok(token) = std::env::var("RELAY_AUTH_TOKEN") {
            config.auth_token = token;
        }
        
        config
    }
    
    /// 保存配置到文件
    pub fn save_to_file(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl AgentConfig {
    /// 从文件加载配置
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// 从环境变量加载
    pub fn from_env() -> Self {
        let mut config = Self::default();
        
        if let Ok(id) = std::env::var("RELAY_DEVICE_ID") {
            config.device_id = id;
        }
        if let Ok(name) = std::env::var("RELAY_DEVICE_NAME") {
            config.device_name = name;
        }
        if let Ok(group) = std::env::var("RELAY_DEVICE_GROUP") {
            config.device_group = group;
        }
        if let Ok(url) = std::env::var("RELAY_SERVER_URL") {
            config.server_url = url;
        }
        if let Ok(token) = std::env::var("RELAY_AUTH_TOKEN") {
            config.auth_token = token;
        }
        
        config
    }
    
    /// 保存配置到文件
    pub fn save_to_file(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
