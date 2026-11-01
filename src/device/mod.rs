//! Hermes Relay - 设备管理模块

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// 设备状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Online,
    Offline,
    Busy,
    Error,
    Maintenance,
}

impl std::fmt::Display for DeviceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Online => write!(f, "online"),
            Self::Offline => write!(f, "offline"),
            Self::Busy => write!(f, "busy"),
            Self::Error => write!(f, "error"),
            Self::Maintenance => write!(f, "maintenance"),
        }
    }
}

/// 设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub device_id: String,
    pub device_name: String,
    pub device_group: String,
    pub os_version: String,
    pub ip_address: String,
    pub mac_address: String,
    pub hostname: String,
    pub capabilities: Vec<String>,
    pub status: DeviceStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub uptime: f64,
    pub active_commands: u32,
    pub websocket_id: String,
    pub connected_at: DateTime<Utc>,
    pub last_command_at: Option<DateTime<Utc>>,
    pub total_commands: u64,
    pub successful_commands: u64,
    pub failed_commands: u64,
}

impl Device {
    /// 创建新设备
    pub fn new(device_id: &str, device_name: &str, device_group: &str) -> Self {
        let now = Utc::now();
        Self {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            device_group: device_group.to_string(),
            os_version: String::new(),
            ip_address: String::new(),
            mac_address: String::new(),
            hostname: String::new(),
            capabilities: Vec::new(),
            status: DeviceStatus::Online,
            last_heartbeat: now,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            disk_usage: 0.0,
            uptime: 0.0,
            active_commands: 0,
            websocket_id: String::new(),
            connected_at: now,
            last_command_at: None,
            total_commands: 0,
            successful_commands: 0,
            failed_commands: 0,
        }
    }
    
    /// 是否在线
    pub fn is_online(&self) -> bool {
        self.status == DeviceStatus::Online
    }
    
    /// 是否忙碌
    pub fn is_busy(&self) -> bool {
        self.status == DeviceStatus::Busy
    }
    
    /// 是否可用
    pub fn is_available(&self) -> bool {
        self.status == DeviceStatus::Online && self.active_commands == 0
    }
    
    /// 命令成功率
    pub fn command_success_rate(&self) -> f64 {
        if self.total_commands == 0 {
            return 100.0;
        }
        (self.successful_commands as f64 / self.total_commands as f64) * 100.0
    }
    
    /// 更新心跳信息
    pub fn update_heartbeat(
        &mut self,
        cpu: f64,
        memory: f64,
        disk: f64,
        uptime: f64,
        active: u32,
    ) {
        self.cpu_usage = cpu;
        self.memory_usage = memory;
        self.disk_usage = disk;
        self.uptime = uptime;
        self.active_commands = active;
        self.last_heartbeat = Utc::now();
        self.status = DeviceStatus::Online;
    }
    
    /// 记录命令结果
    pub fn record_command(&mut self, success: bool) {
        self.total_commands += 1;
        if success {
            self.successful_commands += 1;
        } else {
            self.failed_commands += 1;
        }
        self.last_command_at = Some(Utc::now());
    }
}

/// 设备组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceGroup {
    pub group_id: String,
    pub group_name: String,
    pub description: String,
    pub device_ids: Vec<String>,
}

impl DeviceGroup {
    pub fn new(group_id: &str, group_name: &str) -> Self {
        Self {
            group_id: group_id.to_string(),
            group_name: group_name.to_string(),
            description: String::new(),
            device_ids: Vec::new(),
        }
    }
    
    pub fn add_device(&mut self, device_id: &str) {
        if !self.device_ids.contains(&device_id.to_string()) {
            self.device_ids.push(device_id.to_string());
        }
    }
    
    pub fn remove_device(&mut self, device_id: &str) {
        self.device_ids.retain(|id| id != device_id);
    }
    
    pub fn device_count(&self) -> usize {
        self.device_ids.len()
    }
}

/// 设备管理器
pub struct DeviceManager {
    devices: Arc<RwLock<HashMap<String, Device>>>,
    groups: Arc<RwLock<HashMap<String, DeviceGroup>>>,
    heartbeat_timeout: u64,
}

impl DeviceManager {
    /// 创建新的设备管理器
    pub fn new() -> Self {
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            groups: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_timeout: 60,
        }
    }
    
    /// 设置心跳超时
    pub fn with_heartbeat_timeout(mut self, timeout: u64) -> Self {
        self.heartbeat_timeout = timeout;
        self
    }
    
    /// 注册设备
    pub async fn register_device(
        &self,
        device_id: &str,
        device_name: &str,
        device_group: &str,
        os_version: &str,
        ip_address: &str,
        mac_address: &str,
        hostname: &str,
        capabilities: Vec<String>,
        websocket_id: &str,
    ) -> Device {
        let mut devices = self.devices.write().await;
        
        if let Some(device) = devices.get_mut(device_id) {
            // 更新现有设备
            device.device_name = device_name.to_string();
            device.os_version = os_version.to_string();
            device.ip_address = ip_address.to_string();
            device.mac_address = mac_address.to_string();
            device.hostname = hostname.to_string();
            device.capabilities = capabilities;
            device.websocket_id = websocket_id.to_string();
            device.status = DeviceStatus::Online;
            device.last_heartbeat = Utc::now();
            device.connected_at = Utc::now();
            info!("[DeviceManager] Device updated: {} ({}) in group {}", 
                  device_name, device_id, device_group);
            device.clone()
        } else {
            // 创建新设备
            let mut device = Device::new(device_id, device_name, device_group);
            device.os_version = os_version.to_string();
            device.ip_address = ip_address.to_string();
            device.mac_address = mac_address.to_string();
            device.hostname = hostname.to_string();
            device.capabilities = capabilities;
            device.websocket_id = websocket_id.to_string();
            
            devices.insert(device_id.to_string(), device.clone());
            
            // 添加到组
            let mut groups = self.groups.write().await;
            groups
                .entry(device_group.to_string())
                .or_insert_with(|| DeviceGroup::new(device_group, device_group))
                .add_device(device_id);
            
            info!("[DeviceManager] Device registered: {} ({}) in group {}", 
                  device_name, device_id, device_group);
            device
        }
    }
    
    /// 注销设备
    pub async fn unregister_device(&self, device_id: &str) -> Option<Device> {
        let mut devices = self.devices.write().await;
        if let Some(device) = devices.remove(device_id) {
            // 从组中移除
            let mut groups = self.groups.write().await;
            for group in groups.values_mut() {
                group.remove_device(device_id);
            }
            info!("[DeviceManager] Device unregistered: {} ({})", 
                  device.device_name, device_id);
            Some(device)
        } else {
            None
        }
    }
    
    /// 更新心跳
    pub async fn update_heartbeat(
        &self,
        device_id: &str,
        cpu: f64,
        memory: f64,
        disk: f64,
        uptime: f64,
        active: u32,
    ) -> bool {
        let mut devices = self.devices.write().await;
        if let Some(device) = devices.get_mut(device_id) {
            device.update_heartbeat(cpu, memory, disk, uptime, active);
            true
        } else {
            false
        }
    }
    
    /// 更新设备状态
    pub async fn update_status(&self, device_id: &str, status: DeviceStatus) -> bool {
        let mut devices = self.devices.write().await;
        if let Some(device) = devices.get_mut(device_id) {
            let old_status = device.status.clone();
            device.status = status.clone();
            if old_status != status {
                info!("[DeviceManager] Device {} status: {} -> {}", 
                      device.device_name, old_status, status);
            }
            true
        } else {
            false
        }
    }
    
    /// 记录命令结果
    pub async fn record_command_result(&self, device_id: &str, success: bool) {
        let mut devices = self.devices.write().await;
        if let Some(device) = devices.get_mut(device_id) {
            device.record_command(success);
        }
    }
    
    /// 获取设备
    pub async fn get_device(&self, device_id: &str) -> Option<Device> {
        let devices = self.devices.read().await;
        devices.get(device_id).cloned()
    }
    
    /// 根据名称获取设备
    pub async fn get_device_by_name(&self, name: &str) -> Option<Device> {
        let devices = self.devices.read().await;
        devices.values()
            .find(|d| d.device_name.to_lowercase() == name.to_lowercase())
            .cloned()
    }
    
    /// 获取所有在线设备
    pub async fn get_online_devices(&self) -> Vec<Device> {
        let devices = self.devices.read().await;
        devices.values()
            .filter(|d| d.is_online())
            .cloned()
            .collect()
    }
    
    /// 获取指定组的设备
    pub async fn get_devices_by_group(&self, group_name: &str) -> Vec<Device> {
        let groups = self.groups.read().await;
        let devices = self.devices.read().await;
        
        if let Some(group) = groups.get(group_name) {
            group.device_ids.iter()
                .filter_map(|id| devices.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }
    
    /// 获取所有可用设备
    pub async fn get_available_devices(&self) -> Vec<Device> {
        let devices = self.devices.read().await;
        devices.values()
            .filter(|d| d.is_available())
            .cloned()
            .collect()
    }
    
    /// 获取所有设备
    pub async fn get_all_devices(&self) -> Vec<Device> {
        let devices = self.devices.read().await;
        devices.values().cloned().collect()
    }
    
    /// 获取所有设备组
    pub async fn get_all_groups(&self) -> Vec<DeviceGroup> {
        let groups = self.groups.read().await;
        groups.values().cloned().collect()
    }
    
    /// 设备数量
    pub async fn device_count(&self) -> usize {
        let devices = self.devices.read().await;
        devices.len()
    }
    
    /// 在线设备数量
    pub async fn online_count(&self) -> usize {
        let devices = self.devices.read().await;
        devices.values().filter(|d| d.is_online()).count()
    }
    
    /// 获取设备统计
    pub async fn get_stats(&self) -> DeviceStats {
        let devices = self.devices.read().await;
        let groups = self.groups.read().await;
        
        let total = devices.len();
        let online = devices.values().filter(|d| d.is_online()).count();
        let busy = devices.values().filter(|d| d.is_busy()).count();
        let offline = total - online;
        
        let total_commands: u64 = devices.values().map(|d| d.total_commands).sum();
        let successful_commands: u64 = devices.values().map(|d| d.successful_commands).sum();
        
        DeviceStats {
            total_devices: total,
            online_devices: online,
            busy_devices: busy,
            offline_devices: offline,
            total_groups: groups.len(),
            total_commands,
            successful_commands,
            success_rate: if total_commands > 0 {
                (successful_commands as f64 / total_commands as f64) * 100.0
            } else {
                100.0
            },
        }
    }
    
    /// 检查心跳超时
    pub async fn check_heartbeats(&self) -> Vec<Device> {
        let now = Utc::now();
        let mut offline_devices = Vec::new();
        
        let mut devices = self.devices.write().await;
        for (id, device) in devices.iter_mut() {
            if device.status == DeviceStatus::Online {
                let elapsed = (now - device.last_heartbeat).num_seconds() as u64;
                if elapsed > self.heartbeat_timeout {
                    device.status = DeviceStatus::Offline;
                    offline_devices.push(device.clone());
                    warn!("[DeviceManager] Device {} ({}) offline (no heartbeat for {}s)", 
                          device.device_name, id, elapsed);
                }
            }
        }
        
        offline_devices
    }
}

/// 设备统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStats {
    pub total_devices: usize,
    pub online_devices: usize,
    pub busy_devices: usize,
    pub offline_devices: usize,
    pub total_groups: usize,
    pub total_commands: u64,
    pub successful_commands: u64,
    pub success_rate: f64,
}
