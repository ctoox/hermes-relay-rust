//! Hermes Relay - 主入口

use std::path::PathBuf;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use hermes_relay::{RelayConfig, AgentConfig};

#[derive(Parser)]
#[command(name = "hermes-relay")]
#[command(about = "Hermes Relay - High-performance remote control system")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Server {
        #[arg(short, long)]
        config: Option<PathBuf>,
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        #[arg(long, default_value_t = 8765)]
        port: u16,
    },
    Agent {
        #[arg(short, long)]
        config: Option<PathBuf>,
        #[arg(long)]
        server: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, default_value = "default")]
        group: String,
        #[arg(long)]
        token: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Server { config, host, port } => {
            let mut relay_config = if let Some(path) = config {
                RelayConfig::from_file(&path)?
            } else {
                RelayConfig::from_env()
            };
            relay_config.ws_host = host;
            relay_config.ws_port = port;
            
            let mut server = hermes_relay::server::RelayServer::new(relay_config);
            
            ctrlc::set_handler(|| {
                tracing::info!("Received shutdown signal");
            })?;
            
            server.start().await?;
        }
        
        Commands::Agent { config, server, name, group, token } => {
            let mut agent_config = if let Some(path) = config {
                AgentConfig::from_file(&path)?
            } else {
                AgentConfig::from_env()
            };
            if let Some(url) = server {
                agent_config.server_url = url;
            }
            if let Some(n) = name {
                agent_config.device_name = n;
            }
            agent_config.device_group = group;
            if let Some(t) = token {
                agent_config.auth_token = t;
            }
            
            let agent = hermes_relay::client::AgentClient::new(agent_config);
            
            ctrlc::set_handler(|| {
                tracing::info!("Received shutdown signal");
            })?;
            
            agent.run().await?;
        }
    }
    
    Ok(())
}
