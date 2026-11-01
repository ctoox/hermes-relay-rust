//! Hermes Relay - 命令执行模块

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::process::Command;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tracing::debug;

use crate::protocol::{CommandStatus, CommandPayload, ResponsePayload};

/// 命令请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub command_id: String,
    pub command: String,
    pub normalized_command: String,
    pub device_id: String,
    pub device_group: String,
    pub working_dir: String,
    pub timeout: u64,
    pub env_vars: HashMap<String, String>,
    pub requires_approval: bool,
    pub approval_id: String,
    pub approved: bool,
    pub source: String,
    pub requester: String,
    pub chat_id: String,
    pub status: CommandRequestStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub output: String,
    pub error_output: String,
    pub exit_code: Option<i32>,
    pub execution_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandRequestStatus {
    Pending,
    Approved,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

impl CommandRequest {
    pub fn new(command: &str) -> Self {
        Self {
            command_id: Uuid::new_v4().to_string(),
            command: command.to_string(),
            normalized_command: normalize_command(command),
            device_id: String::new(),
            device_group: String::new(),
            working_dir: String::new(),
            timeout: 30,
            env_vars: HashMap::new(),
            requires_approval: false,
            approval_id: String::new(),
            approved: false,
            source: String::new(),
            requester: String::new(),
            chat_id: String::new(),
            status: CommandRequestStatus::Pending,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            output: String::new(),
            error_output: String::new(),
            exit_code: None,
            execution_time: 0.0,
        }
    }
}

/// 命令结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command_id: String,
    pub status: CommandStatus,
    pub output: String,
    pub error_output: String,
    pub exit_code: Option<i32>,
    pub execution_time: f64,
    pub device_id: String,
}

/// 命令执行器
pub struct CommandExecutor {
    running_commands: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
    semaphore: Arc<Semaphore>,
    default_timeout: u64,
}

impl CommandExecutor {
    pub fn new() -> Self {
        Self {
            running_commands: Arc::new(RwLock::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(5)),
            default_timeout: 60,
        }
    }
    
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.semaphore = Arc::new(Semaphore::new(max));
        self
    }
    
    pub fn with_default_timeout(mut self, timeout: u64) -> Self {
        self.default_timeout = timeout;
        self
    }
    
    pub async fn execute(&self, payload: &CommandPayload) -> ResponsePayload {
        let start = std::time::Instant::now();
        let timeout = if payload.timeout > 0 { payload.timeout } else { self.default_timeout };
        
        let _permit = self.semaphore.acquire().await.unwrap();
        
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout),
            self.run_command(payload),
        ).await;
        
        let execution_time = start.elapsed().as_secs_f64();
        
        match result {
            Ok(Ok((output, error_output, exit_code))) => ResponsePayload {
                command_id: payload.command_id.clone(),
                status: if exit_code == Some(0) { CommandStatus::Success } else { CommandStatus::Error },
                output,
                error_output,
                exit_code,
                execution_time,
            },
            Ok(Err(e)) => ResponsePayload {
                command_id: payload.command_id.clone(),
                status: CommandStatus::Error,
                output: String::new(),
                error_output: e.to_string(),
                exit_code: None,
                execution_time,
            },
            Err(_) => ResponsePayload {
                command_id: payload.command_id.clone(),
                status: CommandStatus::Timeout,
                output: String::new(),
                error_output: "Command execution timed out".to_string(),
                exit_code: None,
                execution_time,
            },
        }
    }
    
    async fn run_command(&self, payload: &CommandPayload) -> anyhow::Result<(String, String, Option<i32>)> {
        let mut cmd = Command::new("cmd");
        cmd.args(&["/C", &payload.command]);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        if !payload.working_dir.is_empty() {
            cmd.current_dir(&payload.working_dir);
        }
        
        for (key, value) in &payload.env_vars {
            cmd.env(key, value);
        }
        
        let output = cmd.output().await?;
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();
        
        Ok((stdout, stderr, exit_code))
    }
    
    pub async fn cancel(&self, command_id: &str) -> bool {
        let mut commands = self.running_commands.write().await;
        if let Some(handle) = commands.remove(command_id) {
            handle.abort();
            true
        } else {
            false
        }
    }
    
    pub async fn active_count(&self) -> usize {
        let commands = self.running_commands.read().await;
        commands.len()
    }
}

/// 命令引擎
pub struct CommandEngine {
    executor: Arc<CommandExecutor>,
    pending_commands: Arc<RwLock<HashMap<String, CommandRequest>>>,
    executing_commands: Arc<RwLock<HashMap<String, CommandRequest>>>,
    completed_commands: Arc<RwLock<HashMap<String, CommandResult>>>,
}

impl CommandEngine {
    pub fn new() -> Self {
        Self {
            executor: Arc::new(CommandExecutor::new()),
            pending_commands: Arc::new(RwLock::new(HashMap::new())),
            executing_commands: Arc::new(RwLock::new(HashMap::new())),
            completed_commands: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub fn with_executor(executor: CommandExecutor) -> Self {
        Self {
            executor: Arc::new(executor),
            pending_commands: Arc::new(RwLock::new(HashMap::new())),
            executing_commands: Arc::new(RwLock::new(HashMap::new())),
            completed_commands: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn process_command(
        &self,
        command: &str,
        device_id: &str,
        device_group: &str,
        working_dir: &str,
        timeout: u64,
        env_vars: HashMap<String, String>,
        requester: &str,
        chat_id: &str,
    ) -> CommandRequest {
        let mut request = CommandRequest::new(command);
        request.device_id = device_id.to_string();
        request.device_group = device_group.to_string();
        request.working_dir = working_dir.to_string();
        request.timeout = timeout;
        request.env_vars = env_vars;
        request.requester = requester.to_string();
        request.chat_id = chat_id.to_string();
        
        let command_id = request.command_id.clone();
        
        let mut pending = self.pending_commands.write().await;
        pending.insert(command_id, request.clone());
        
        request
    }
    
    pub async fn execute_command(&self, command_id: &str) -> Option<CommandResult> {
        let mut pending = self.pending_commands.write().await;
        if let Some(mut request) = pending.remove(command_id) {
            request.status = CommandRequestStatus::Executing;
            request.started_at = Some(Utc::now());
            
            let mut executing = self.executing_commands.write().await;
            executing.insert(command_id.to_string(), request);
            
            drop(executing);
            drop(pending);
            
            let payload = CommandPayload {
                command_id: command_id.to_string(),
                command: String::new(), // Will be filled from request
                working_dir: String::new(),
                timeout: 30,
                env_vars: HashMap::new(),
            };
            
            let response = self.executor.execute(&payload).await;
            
            let result = CommandResult {
                command_id: command_id.to_string(),
                status: response.status,
                output: response.output,
                error_output: response.error_output,
                exit_code: response.exit_code,
                execution_time: response.execution_time,
                device_id: String::new(),
            };
            
            let mut executing = self.executing_commands.write().await;
            executing.remove(command_id);
            
            let mut completed = self.completed_commands.write().await;
            completed.insert(command_id.to_string(), result.clone());
            
            Some(result)
        } else {
            None
        }
    }
    
    pub async fn get_result(&self, command_id: &str) -> Option<CommandResult> {
        let completed = self.completed_commands.read().await;
        completed.get(command_id).cloned()
    }
    
    pub async fn get_pending_count(&self) -> usize {
        let pending = self.pending_commands.read().await;
        pending.len()
    }
    
    pub async fn get_executing_count(&self) -> usize {
        let executing = self.executing_commands.read().await;
        executing.len()
    }
}

fn normalize_command(command: &str) -> String {
    let mut cmd = command.trim().to_string();
    cmd = cmd.replace('/', "\\");
    // Remove multiple spaces
    while cmd.contains("  ") {
        cmd = cmd.replace("  ", " ");
    }
    cmd
}
