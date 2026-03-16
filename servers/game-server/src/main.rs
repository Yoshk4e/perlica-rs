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
    // Yo, this is where the server kicks off! Let's get this party started.
    init_tracing(tracing::Level::DEBUG);

    let cfg = crate::sconfig::Config::load()?; // Loading up the server config. Gotta know what we're workin' with, right?
    info!(addr = %cfg.server.addr());

    let assets = BeyondAssets::load(&cfg.assets.path)?;
    let assets: &'static BeyondAssets = Box::leak(Box::new(assets));

    let registry = SessionRegistry::new();
    let registry: &'static SessionRegistry = Box::leak(Box::new(registry));

    let db = PlayerDb::open("saves")?;

    let db: &'static PlayerDb = Box::leak(Box::new(db));

    let listener = TcpListener::bind(cfg.server.addr()).await?; // Binding to the port. Hope nobody else is using it, lol.
    info!(addr = %listener.local_addr()?, "listening"); // Server's up! Time to tell the world (or at least the console).

    loop {
        // Infinite loop for handling connections. We're always open for business! XD
        match listener.accept().await {
            // New connection! Who dis?
            Ok((socket, addr)) => {
                // If it's all good, let's handle this connection.
                info!(addr = %addr, "connected"); // Log it, so we know who's knocking.
                tokio::spawn(async move {
                    // Spawning a new task for each connection. Don't wanna block the main thread, ya know?
                    if let Err(e) = net::handle_connection(socket, addr, assets, registry, db).await
                    // Let the session begin! Hope it's a good one.
                    {
                        error!(addr = %addr, error = %e, "connection error"); // Uh oh, something went wrong. Better log it.
                    }
                });
            }
            Err(e) => {
                warn!(error = %e, "accept failed"); // Connection failed. Probably just a rando bot, lol.
            }
        }
    }
}
