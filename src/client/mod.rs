//! Hermes Relay - 客户端模块

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use tracing::{info, warn, error};
use serde_json::json;
use sysinfo::{System, SystemExt, CpuExt};

use crate::config::AgentConfig;
use crate::command::CommandExecutor;
use crate::protocol::*;
use crate::error::{RelayError, Result};

pub struct AgentClient {
    config: AgentConfig,
    executor: Arc<CommandExecutor>,
    connected: Arc<RwLock<bool>>,
}

impl AgentClient {
    pub fn new(config: AgentConfig) -> Self {
        let executor = Arc::new(
            CommandExecutor::new()
                .with_max_concurrent(config.max_concurrent_commands as usize)
                .with_default_timeout(config.command_timeout)
        );
        Self {
            config,
            executor,
            connected: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn run(&self) -> Result<()> {
        info!("========================================");
        info!("Hermes Relay Agent starting...");
        info!("========================================");
        info!("Device: {} ({})", self.config.device_name, self.config.device_id);
        info!("Server: {}", self.config.server_url);
        
        let mut reconnect_count = 0;
        let max_reconnect = self.config.max_reconnect_attempts;
        
        loop {
            match self.try_connect().await {
                Ok(_) => {
                    reconnect_count = 0;
                }
                Err(e) => {
                    error!("[Connection] Error: {}", e);
                    reconnect_count += 1;
                    if max_reconnect > 0 && reconnect_count >= max_reconnect {
                        error!("[Connection] Max reconnect attempts reached");
                        return Err(e);
                    }
                }
            }
            
            warn!("[Connection] Reconnecting in {}s...", self.config.reconnect_interval);
            tokio::time::sleep(tokio::time::Duration::from_secs(self.config.reconnect_interval)).await;
        }
    }
    
    async fn try_connect(&self) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.config.server_url).await
            .map_err(|e| RelayError::Connection(e.to_string()))?;
        info!("[Connection] Connected to {}", self.config.server_url);
        
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();
        
        *self.connected.write().await = true;
        
        let register = ProtocolMessage::new(MessageType::Register, json!({
            "device_id": self.config.device_id,
            "device_name": self.config.device_name,
            "device_group": self.config.device_group,
            "os_version": get_os_version(),
            "ip_address": get_local_ip(),
            "hostname": get_hostname(),
            "capabilities": ["command", "screenshot"],
            "auth_token": self.config.auth_token,
        }));
        
        tx.send(WsMessage::Text(register.to_json().map_err(|e| RelayError::Json(e))?)).ok();
        
        let send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_sender.send(msg).await.is_err() {
                    break;
                }
            }
        });
        
        let executor = self.executor.clone();
        let connected = self.connected.clone();
        let tx_recv = tx.clone();
        
        let recv_task = tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(WsMessage::Text(text)) => {
                        if let Ok(message) = ProtocolMessage::from_json(&text) {
                            handle_server_message(&message, &executor, &tx_recv).await;
                        }
                    }
                    Ok(WsMessage::Close(_)) => {
                        info!("[Connection] Server closed connection");
                        break;
                    }
                    Err(e) => {
                        error!("[Connection] Receive error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
            *connected.write().await = false;
        });
        
        let device_id = self.config.device_id.clone();
        let device_name = self.config.device_name.clone();
        let heartbeat_interval = self.config.heartbeat_interval;
        
        let heartbeat_task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(heartbeat_interval)).await;
                
                let sys = System::new_all();
                let cpu = sys.global_cpu_info().cpu_usage() as f64;
                let memory = if sys.total_memory() > 0 {
                    (sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0
                } else {
                    0.0
                };
                
                let heartbeat = ProtocolMessage::new(MessageType::Heartbeat, json!({
                    "device_id": device_id,
                    "device_name": device_name,
                    "device_group": "default",
                    "cpu_usage": cpu,
                    "memory_usage": memory,
                    "disk_usage": 0.0,
                    "uptime": 0.0,
                    "active_commands": 0,
                }));
                
                if tx.send(WsMessage::Text(heartbeat.to_json().unwrap_or_default())).is_err() {
                    break;
                }
            }
        });
        
        tokio::select! {
            _ = send_task => {}
            _ = recv_task => {}
            _ = heartbeat_task => {}
        }
        
        Ok(())
    }
}

async fn handle_server_message(
    message: &ProtocolMessage,
    executor: &Arc<CommandExecutor>,
    tx: &mpsc::UnboundedSender<WsMessage>,
) {
    match message.msg_type {
        MessageType::RegisterAck => {
            info!("[Connection] Registration acknowledged");
        }
        MessageType::Command => {
            if let Ok(payload) = serde_json::from_value::<CommandPayload>(message.payload.clone()) {
                info!("[Command] Executing: {}", payload.command);
                let response = executor.execute(&payload).await;
                let resp_msg = ProtocolMessage::new(MessageType::Response, serde_json::to_value(&response).unwrap_or_default());
                tx.send(WsMessage::Text(resp_msg.to_json().unwrap_or_default())).ok();
            }
        }
        MessageType::Cancel => {
            if let Ok(payload) = serde_json::from_value::<CancelPayload>(message.payload.clone()) {
                executor.cancel(&payload.command_id).await;
            }
        }
        MessageType::Pong => {}
        _ => {}
    }
}

fn get_os_version() -> String {
    let sys = System::new_all();
    let name = sys.name().unwrap_or_else(|| "Windows".to_string());
    let version = sys.os_version().unwrap_or_else(|| "10".to_string());
    format!("{} {}", name, version)
}

fn get_hostname() -> String {
    hostname::get().map(|h| h.to_string_lossy().to_string()).unwrap_or_else(|_| "unknown".to_string())
}

fn get_local_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("8.8.8.8:80").and_then(|_| s.local_addr()))
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}
