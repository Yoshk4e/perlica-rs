mod handlers;
mod net;
mod player;
mod sconfig;

use common::logging::init_tracing;
use config::BeyondAssets;
use net::SessionRegistry;
use perlica_db::PlayerDb;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing(tracing::Level::DEBUG);

    let cfg = crate::sconfig::Config::load()?;
    info!(addr = %cfg.server.addr());

    let assets = BeyondAssets::load(&cfg.assets.path)?;
    let assets: &'static BeyondAssets = Box::leak(Box::new(assets));

    let registry = SessionRegistry::new();
    let registry: &'static SessionRegistry = Box::leak(Box::new(registry));

    let db = PlayerDb::open("saves")?;

    let db: &'static PlayerDb = Box::leak(Box::new(db));

    let listener = TcpListener::bind(cfg.server.addr()).await?;
    info!(addr = %listener.local_addr()?, "listening");

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                info!(addr = %addr, "connected");
                tokio::spawn(async move {
                    if let Err(e) = net::handle_connection(socket, addr, assets, registry, db).await
                    {
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
