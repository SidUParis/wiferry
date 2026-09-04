mod network;
mod range;
mod state;
mod web;

use anyhow::Context;
use clap::{Parser, ValueEnum};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "wiferry",
    version,
    about = "Deliver files over the local network to any browser"
)]
struct Cli {
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,

    #[arg(short = 'f', long = "file", value_name = "PATH")]
    files: Vec<PathBuf>,

    #[arg(long, default_value_t = 8765)]
    port: u16,

    #[arg(long)]
    host_ip: Option<Ipv4Addr>,

    #[arg(long, value_enum, default_value_t = TransportChoice::Auto)]
    transport: TransportChoice,

    #[arg(long)]
    name: Option<String>,

    #[arg(long, default_value_t = 30)]
    expiry: u64,

    #[arg(long)]
    no_browser: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TransportChoice {
    Auto,
    Lan,
    Tailscale,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "wiferry=info".into()),
        )
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();
    if cli.port == 0 {
        anyhow::bail!("--port must be between 1 and 65535");
    }
    if !matches!(cli.expiry, 0 | 15 | 30 | 60 | 120) {
        anyhow::bail!("--expiry must be one of: 0, 15, 30, 60, 120");
    }
    let probe_tailscale_cli = !matches!(cli.transport, TransportChoice::Lan);
    let mut candidates = network::candidates(probe_tailscale_cli)
        .context("cannot enumerate local IPv4 interfaces")?;
    let requested_transport = match cli.transport {
        TransportChoice::Auto => None,
        TransportChoice::Lan => Some(network::TransportKind::Lan),
        TransportChoice::Tailscale => Some(network::TransportKind::Tailscale),
    };
    let host = network::select_candidate(&candidates, cli.host_ip, requested_transport)
        .map_err(anyhow::Error::msg)?;
    if !candidates
        .iter()
        .any(|candidate| candidate.address == host.address)
    {
        candidates.insert(0, host.clone());
    }
    let device_name = cli.name.unwrap_or_else(default_device_name);
    let admin_listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .context("cannot bind the loopback management listener")?;
    let admin_port = admin_listener.local_addr()?.port();
    let state = Arc::new(state::AppState::new(
        host,
        candidates,
        cli.port,
        admin_port,
        device_name,
        cli.expiry,
    )?);
    for path in cli.paths.iter().chain(cli.files.iter()) {
        state
            .add_path(path, false)
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("cannot share {}", path.display()))?;
    }

    let guest_address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, cli.port));
    let guest_listener = tokio::net::TcpListener::bind(guest_address)
        .await
        .with_context(|| format!("cannot listen on {guest_address}"))?;
    let admin_url = format!("http://127.0.0.1:{admin_port}/#{}", state.admin_token());
    println!("Wiferry Rust management: {admin_url}");
    println!("Nearby devices:          {}", state.share_url());
    println!("Data plane: Rust async streaming · 128 KiB chunks · transport-scoped guard");
    println!("Press Ctrl+C to stop sharing.");
    if !cli.no_browser {
        let url = admin_url.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = webbrowser::open(&url);
        });
    }

    let guest_app = web::guest_router(state.clone());
    let admin_app = web::admin_router(state);
    let guest_server = axum::serve(
        guest_listener,
        guest_app.into_make_service_with_connect_info::<SocketAddr>(),
    );
    let admin_server = axum::serve(
        admin_listener,
        admin_app.into_make_service_with_connect_info::<SocketAddr>(),
    );
    tokio::select! {
        result = guest_server => result?,
        result = admin_server => result?,
        _ = shutdown() => {},
    }
    Ok(())
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}

fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Wiferry host".into())
        .split('.')
        .next()
        .unwrap_or("Wiferry host")
        .to_string()
}
