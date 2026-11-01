//! Hermes Relay - 服务端模块

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};
use tracing::{info, warn, error, debug};
use serde_json::json;

use crate::config::RelayConfig;
use crate::device::{DeviceManager, DeviceStatus};
use crate::command::CommandEngine;
use crate::nlp::NlpParser;
use crate::feishu::FeishuAdapter;
use crate::protocol::*;
use crate::error::{RelayError, Result};

struct Connection {
    device_id: Option<String>,
    sender: mpsc::UnboundedSender<WsMessage>,
}

pub struct RelayServer {
    config: RelayConfig,
    device_manager: Arc<DeviceManager>,
    command_engine: Arc<CommandEngine>,
    nlp_parser: Arc<NlpParser>,
    feishu: Arc<FeishuAdapter>,
    connections: Arc<RwLock<HashMap<String, Connection>>>,
}

impl RelayServer {
    pub fn new(config: RelayConfig) -> Self {
        let device_manager = Arc::new(
            DeviceManager::new().with_heartbeat_timeout(config.heartbeat_timeout)
        );
        let feishu = Arc::new(FeishuAdapter::new(config.feishu.clone()));
        
        Self {
            config,
            device_manager,
            command_engine: Arc::new(CommandEngine::new()),
            nlp_parser: Arc::new(NlpParser::new()),
            feishu,
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn start(&mut self) -> Result<()> {
        info!("========================================");
        info!("Hermes Relay Server starting...");
        info!("========================================");
        
        let device_manager = self.device_manager.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                let offline = device_manager.check_heartbeats().await;
                for device in offline {
                    warn!("[Heartbeat] Device {} offline", device.device_name);
                }
            }
        });
        
        if !self.feishu.connect().await? {
            error!("Failed to connect to Feishu");
            return Err(RelayError::Connection("Failed to connect to Feishu".to_string()));
        }
        
        let addr = format!("{}:{}", self.config.ws_host, self.config.ws_port);
        let listener = TcpListener::bind(&addr).await?;
        info!("[WebSocket] Server listening on {}", addr);
        
        info!("========================================");
        info!("Hermes Relay Server started successfully!");
        info!("WebSocket: {}", addr);
        info!("Feishu: Connected ({})", self.config.feishu.domain);
        info!("========================================");
        
        loop {
            let (stream, addr) = listener.accept().await?;
            info!("[WebSocket] New connection from {}", addr);
            
            let connections = self.connections.clone();
            let device_manager = self.device_manager.clone();
            let command_engine = self.command_engine.clone();
            
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, connections, device_manager, command_engine).await {
                    error!("[WebSocket] Connection error: {}", e);
                }
            });
        }
    }
    
    pub async fn stop(&self) {
        info!("Stopping Hermes Relay Server...");
        self.feishu.disconnect().await;
        info!("Hermes Relay Server stopped");
    }
}

async fn handle_connection(
    stream: TcpStream,
    connections: Arc<RwLock<HashMap<String, Connection>>>,
    device_manager: Arc<DeviceManager>,
    command_engine: Arc<CommandEngine>,
) -> Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();
    
    let connection_id = uuid::Uuid::new_v4().to_string();
    let connection = Connection {
        device_id: None,
        sender: tx.clone(),
    };
    
    connections.write().await.insert(connection_id.clone(), connection);
    
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });
    
    let connections_recv = connections.clone();
    let device_manager_recv = device_manager.clone();
    let connection_id_recv = connection_id.clone();
    let tx_recv = tx.clone();
    
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    if let Err(e) = handle_message(
                        &text,
                        &connection_id_recv,
                        &connections_recv,
                        &device_manager_recv,
                        &command_engine,
                        &tx_recv,
                    ).await {
                        error!("[WebSocket] Message handling error: {}", e);
                    }
                }
                Ok(WsMessage::Close(_)) => {
                    info!("[WebSocket] Connection closed");
                    break;
                }
                Err(e) => {
                    error!("[WebSocket] Receive error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });
    
    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }
    
    let mut conns = connections.write().await;
    if let Some(conn) = conns.remove(&connection_id) {
        if let Some(device_id) = conn.device_id {
            device_manager.update_status(&device_id, DeviceStatus::Offline).await;
        }
    }
    
    Ok(())
}

async fn handle_message(
    text: &str,
    connection_id: &str,
    connections: &Arc<RwLock<HashMap<String, Connection>>>,
    device_manager: &Arc<DeviceManager>,
    _command_engine: &Arc<CommandEngine>,
    tx: &mpsc::UnboundedSender<WsMessage>,
) -> Result<()> {
    let message: ProtocolMessage = serde_json::from_str(text)?;
    
    match message.msg_type {
        MessageType::Register => {
            let payload: RegisterPayload = serde_json::from_value(message.payload)?;
            
            let device = device_manager.register_device(
                &payload.device_id,
                &payload.device_name,
                &payload.device_group,
                &payload.os_version,
                &payload.ip_address,
                &payload.mac_address,
                &payload.hostname,
                payload.capabilities,
                connection_id,
            ).await;
            
            let mut conns = connections.write().await;
            if let Some(conn) = conns.get_mut(connection_id) {
                conn.device_id = Some(payload.device_id.clone());
            }
            
            let ack = ProtocolMessage::new(MessageType::RegisterAck, json!({
                "device_id": device.device_id,
                "status": "ok",
                "server_time": chrono::Utc::now().timestamp() as f64,
            }));
            
            let _ = tx.send(WsMessage::Text(ack.to_json()?));
        }
        
        MessageType::Heartbeat => {
            let payload: HeartbeatPayload = serde_json::from_value(message.payload)?;
            
            device_manager.update_heartbeat(
                &payload.device_id,
                payload.cpu_usage,
                payload.memory_usage,
                payload.disk_usage,
                payload.uptime,
                payload.active_commands,
            ).await;
            
            let pong = ProtocolMessage::new(MessageType::Pong, json!({
                "server_time": chrono::Utc::now().timestamp() as f64,
            }));
            
            let _ = tx.send(WsMessage::Text(pong.to_json()?));
        }
        
        MessageType::Response => {
            let payload: ResponsePayload = serde_json::from_value(message.payload)?;
            debug!("[Command] Response for {}: {:?}", payload.command_id, payload.status);
        }
        
        MessageType::Pong => {}
        
        _ => {
            warn!("[WebSocket] Unknown message type: {:?}", message.msg_type);
        }
    }
    
    Ok(())
}
