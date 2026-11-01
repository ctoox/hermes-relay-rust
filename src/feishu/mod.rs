//! Hermes Relay - 飞书通信模块

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::Utc;
use reqwest::Client;
use tracing::{info, error, debug};

use crate::config::FeishuConfig;
use crate::error::{RelayError, Result};

/// 飞书消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessage {
    pub message_id: String,
    pub chat_id: String,
    pub chat_type: String,
    pub sender_id: String,
    pub sender_type: String,
    pub content: String,
    pub message_type: String,
    pub mentions: Vec<MentionInfo>,
    pub create_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentionInfo {
    pub key: String,
    pub id: String,
    pub name: String,
}

/// 飞书 API 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuApiResponse<T> {
    pub code: i32,
    pub msg: String,
    pub data: Option<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageData {
    pub message_id: String,
}

/// 飞书适配器
pub struct FeishuAdapter {
    config: FeishuConfig,
    http_client: Client,
    tenant_access_token: Arc<RwLock<String>>,
    connected: Arc<RwLock<bool>>,
    seen_messages: Arc<RwLock<HashMap<String, f64>>>,
}

impl FeishuAdapter {
    pub fn new(config: FeishuConfig) -> Self {
        Self {
            config,
            http_client: Client::new(),
            tenant_access_token: Arc::new(RwLock::new(String::new())),
            connected: Arc::new(RwLock::new(false)),
            seen_messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 获取 API 基础 URL
    fn base_url(&self) -> String {
        match self.config.domain.as_str() {
            "lark" => "https://open.larksuite.com/open-apis".to_string(),
            _ => "https://open.feishu.cn/open-apis".to_string(),
        }
    }
    
    /// 获取 tenant_access_token
    async fn get_tenant_access_token(&self) -> Result<String> {
        let url = format!("{}/auth/v3/tenant_access_token/internal", self.base_url());
        
        let body = json!({
            "app_id": self.config.app_id,
            "app_secret": self.config.app_secret,
        });
        
        let response = self.http_client.post(&url)
            .json(&body)
            .send()
            .await?;
        
        let result: serde_json::Value = response.json().await?;
        
        if result["code"].as_i64().unwrap_or(0) == 0 {
            let token = result["tenant_access_token"].as_str().unwrap_or("").to_string();
            let mut current_token = self.tenant_access_token.write().await;
            *current_token = token.clone();
            Ok(token)
        } else {
            Err(RelayError::FeishuApi {
                code: result["code"].as_i64().unwrap_or(0) as i32,
                message: result["msg"].as_str().unwrap_or("Unknown error").to_string(),
            })
        }
    }
    
    /// 连接到飞书
    pub async fn connect(&self) -> Result<bool> {
        if self.config.app_id.is_empty() || self.config.app_secret.is_empty() {
            error!("[Feishu] FEISHU_APP_ID or FEISHU_APP_SECRET not set");
            return Ok(false);
        }
        
        info!("[Feishu] Connecting to {}...", self.config.domain);
        
        // 获取 access token
        match self.get_tenant_access_token().await {
            Ok(_) => {
                let mut connected = self.connected.write().await;
                *connected = true;
                info!("[Feishu] Connected successfully");
                Ok(true)
            }
            Err(e) => {
                error!("[Feishu] Failed to connect: {}", e);
                Ok(false)
            }
        }
    }
    
    /// 检查是否重复消息
    async fn is_duplicate(&self, message_id: &str) -> bool {
        let now = Utc::now().timestamp() as f64;
        let mut seen = self.seen_messages.write().await;
        
        // 清理过期记录 (24小时)
        let ttl = 86400.0;
        seen.retain(|_, v| now - *v < ttl);
        
        // 检查是否已存在
        if seen.contains_key(message_id) {
            return true;
        }
        
        // 记录新消息
        seen.insert(message_id.to_string(), now);
        
        // 限制缓存大小
        if seen.len() > 2048 {
            let oldest_key = seen.iter()
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(k, _)| k.clone());
            if let Some(key) = oldest_key {
                seen.remove(&key);
            }
        }
        
        false
    }
    
    /// 发送消息
    pub async fn send_message(
        &self,
        chat_id: &str,
        content: &str,
        message_type: &str,
    ) -> Result<Option<String>> {
        let token = self.tenant_access_token.read().await;
        if token.is_empty() {
            return Err(RelayError::Auth("Not connected".to_string()));
        }
        
        let url = format!("{}/im/v1/messages?receive_id_type=chat_id", self.base_url());
        
        let content_value = match message_type {
            "text" => json!({"text": content}),
            "post" => self.build_post_content(content),
            "interactive" => serde_json::from_str(content).unwrap_or(json!({})),
            _ => json!({"text": content}),
        };
        
        let body = json!({
            "receive_id": chat_id,
            "msg_type": message_type,
            "content": serde_json::to_string(&content_value).unwrap_or_default(),
        });
        
        let response = self.http_client.post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await?;
        
        let result: serde_json::Value = response.json().await?;
        
        if result["code"].as_i64().unwrap_or(0) == 0 {
            let message_id = result["data"]["message_id"].as_str().unwrap_or("").to_string();
            debug!("[Feishu] Message sent: {}", message_id);
            Ok(Some(message_id))
        } else {
            error!("[Feishu] Send failed: {}", result["msg"]);
            Ok(None)
        }
    }
    
    /// 回复消息
    pub async fn reply_message(
        &self,
        message_id: &str,
        content: &str,
        message_type: &str,
    ) -> Result<Option<String>> {
        let token = self.tenant_access_token.read().await;
        if token.is_empty() {
            return Err(RelayError::Auth("Not connected".to_string()));
        }
        
        let url = format!("{}/im/v1/messages/{}/reply", self.base_url(), message_id);
        
        let content_value = match message_type {
            "text" => json!({"text": content}),
            _ => json!({"text": content}),
        };
        
        let body = json!({
            "msg_type": message_type,
            "content": serde_json::to_string(&content_value).unwrap_or_default(),
        });
        
        let response = self.http_client.post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await?;
        
        let result: serde_json::Value = response.json().await?;
        
        if result["code"].as_i64().unwrap_or(0) == 0 {
            let message_id = result["data"]["message_id"].as_str().unwrap_or("").to_string();
            Ok(Some(message_id))
        } else {
            error!("[Feishu] Reply failed: {}", result["msg"]);
            Ok(None)
        }
    }
    
    /// 更新消息
    pub async fn update_message(
        &self,
        message_id: &str,
        content: &str,
        message_type: &str,
    ) -> Result<bool> {
        let token = self.tenant_access_token.read().await;
        if token.is_empty() {
            return Err(RelayError::Auth("Not connected".to_string()));
        }
        
        let url = format!("{}/im/v1/messages/{}", self.base_url(), message_id);
        
        let content_value = match message_type {
            "text" => json!({"text": content}),
            _ => json!({"text": content}),
        };
        
        let body = json!({
            "msg_type": message_type,
            "content": serde_json::to_string(&content_value).unwrap_or_default(),
        });
        
        let response = self.http_client.put(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await?;
        
        let result: serde_json::Value = response.json().await?;
        
        Ok(result["code"].as_i64().unwrap_or(0) == 0)
    }
    
    /// 发送卡片
    pub async fn send_card(
        &self,
        chat_id: &str,
        card: &serde_json::Value,
    ) -> Result<Option<String>> {
        let content = serde_json::to_string(card).unwrap_or_default();
        self.send_message(chat_id, &content, "interactive").await
    }
    
    /// 构建富文本内容
    fn build_post_content(&self, text: &str) -> serde_json::Value {
        let lines: Vec<Vec<serde_json::Value>> = text.lines()
            .map(|line| {
                if line.trim().is_empty() {
                    vec![json!({"tag": "text", "text": " "})]
                } else {
                    vec![json!({"tag": "text", "text": line})]
                }
            })
            .collect();
        
        json!({
            "zh_cn": {
                "content": lines
            }
        })
    }
    
    /// 断开连接
    pub async fn disconnect(&self) {
        let mut connected = self.connected.write().await;
        *connected = false;
        info!("[Feishu] Disconnected");
    }
    
    /// 是否已连接
    pub async fn is_connected(&self) -> bool {
        let connected = self.connected.read().await;
        *connected
    }
}
