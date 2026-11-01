//! Hermes Relay - 服务端入口

use std::path::PathBuf;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use hermes_relay::{RelayConfig, server::RelayServer};

#[derive(Parser)]
#[command(name = "hermes-relay-server")]
#[command(about = "Hermes Relay Server - Remote control via Feishu")]
struct Cli {
    #[arg(short, long, help = "Config file path")]
    config: Option<PathBuf>,
    
    #[arg(long, default_value = "0.0.0.0", help = "Listen address")]
    host: String,
    
    #[arg(long, default_value_t = 8765, help = "Listen port")]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();
    
    let cli = Cli::parse();
    
    let mut config = if let Some(path) = cli.config {
        RelayConfig::from_file(&path)?
    } else {
        RelayConfig::from_env()
    };
    
    config.ws_host = cli.host;
    config.ws_port = cli.port;
    
    let mut server = RelayServer::new(config);
    
    ctrlc::set_handler(|| {
        tracing::info!("Received shutdown signal");
    })?;
    
    server.start().await?;
    
    Ok(())
}
