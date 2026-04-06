mod error;
mod gm;
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

    if cfg.muip.enabled {
        let admin_addr = cfg.muip.addr();
        tokio::spawn(async move {
            if let Err(error) = gm::run_gm_listener(admin_addr, registry).await {
                error!("MUIP GM listener failed: {}", error);
            }
        });
    }

    let listener = TcpListener::bind(cfg.server.addr()).await?;
    info!("Listening {}", listener.local_addr()?);

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                info!("Connected {}", addr);
                tokio::spawn(async move {
                    if let Err(e) = net::handle_connection(socket, assets, registry, db).await {
                        error!("Connection Error {}, {}", addr, e);
                    }
                });
            }
            Err(e) => {
                warn!("Accept Failed: {}", e);
            }
        }
    }
}
