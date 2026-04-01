mod error;
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
async fn main() -> Result<(), error::ServerError> {
    init_tracing(tracing::Level::DEBUG);

    let cfg = crate::sconfig::Config::load()?;
    info!("addr From Config: {}", cfg.server.addr());

    let assets = BeyondAssets::load(&cfg.assets.path)?;
    let assets: &'static BeyondAssets = Box::leak(Box::new(assets));

    let registry = SessionRegistry::new();
    let registry: &'static SessionRegistry = Box::leak(Box::new(registry));

    let db = PlayerDb::open("saves")?;

    let db: &'static PlayerDb = Box::leak(Box::new(db));

    let listener = TcpListener::bind(cfg.server.addr()).await?;
    info!("Listening {}", listener.local_addr()?);

    loop {
        match listener.accept().await {
            // New connection! Who dis?
            Ok((socket, addr)) => {
                // If it's all good, let's handle this connection.
                info!("Connected {}", addr);
                tokio::spawn(async move {
                    if let Err(e) = net::handle_connection(socket, assets, registry, db).await
                    // Let the session begin! Hope it's a good one.
                    {
                        error!("Connection Error {}, {}", addr, e); // Uh oh, something went wrong. Better log it.
                    }
                });
            }
            Err(e) => {
                warn!("Accept Failed: {}", e); // Connection failed. Probably just a rando bot, lol.
            }
        }
    }
}
