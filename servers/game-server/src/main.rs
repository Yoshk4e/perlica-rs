mod handlers;
mod net;
mod player;

use common::logging::init_tracing;
use config::BeyondAssets;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing(tracing::Level::DEBUG);

    let assets = BeyondAssets::load("assets")?;
    let assets: &'static BeyondAssets = Box::leak(Box::new(assets));

    let listener = TcpListener::bind("0.0.0.0:1337").await?;
    info!(addr = %listener.local_addr()?, "listening");

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                info!(addr = %addr, "connected");
                tokio::spawn(async move {
                    if let Err(e) = net::handle_connection(socket, addr, assets).await {
                        error!(addr = %addr, error = %e, "connection error");
                    }
                });
            }
            Err(e) => {
                warn!(error = %e, "accept failed");
            }
        }
    }
}
