//! Hermes Relay - 被控端入口

use std::path::PathBuf;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use hermes_relay::{AgentConfig, client::AgentClient};

#[derive(Parser)]
#[command(name = "hermes-relay-agent")]
#[command(about = "Hermes Relay Agent - Remote control agent")]
struct Cli {
    #[arg(short, long, help = "Config file path")]
    config: Option<PathBuf>,
    
    #[arg(long, help = "Server URL (ws://host:port)")]
    server: Option<String>,
    
    #[arg(long, help = "Device name")]
    name: Option<String>,
    
    #[arg(long, default_value = "default", help = "Device group")]
    group: String,
    
    #[arg(long, help = "Auth token")]
    token: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();
    
    let cli = Cli::parse();
    
    let mut config = if let Some(path) = cli.config {
        AgentConfig::from_file(&path)?
    } else {
        AgentConfig::from_env()
    };
    
    if let Some(url) = cli.server {
        config.server_url = url;
    }
    if let Some(name) = cli.name {
        config.device_name = name;
    }
    config.device_group = cli.group;
    if let Some(token) = cli.token {
        config.auth_token = token;
    }
    
    let agent = AgentClient::new(config);
    
    ctrlc::set_handler(|| {
        tracing::info!("Received shutdown signal");
    })?;
    
    agent.run().await?;
    
    Ok(())
}
