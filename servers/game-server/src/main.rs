mod handler;
mod player;
mod session;

use common::logging::init_tracing;
use config::BeyondAssets;
use tokio::net::TcpListener;
use tracing::{debug, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing(tracing::Level::DEBUG);
    let assets = BeyondAssets::load("assets")?;
    let assets: &'static BeyondAssets = Box::leak(Box::new(assets));

    let listener = TcpListener::bind("0.0.0.0:1337").await?;
    debug!("Server listening on 0.0.0.0:1337");

    loop {
        let (socket, addr) = listener.accept().await?;
        debug!("New connection from: {}", addr);

        tokio::spawn(async move {
            if let Err(e) = session::handle_connection(socket, assets).await {
                error!("Error handling connection from {}: {}", addr, e);
            }
        });
    }
}
