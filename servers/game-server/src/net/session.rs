use crate::net::{
    context::NetContext,
    notify::{Notification, PlayerHandle},
    registry::SessionRegistry,
    router::handle_command,
};
use crate::player::Player;
use config::BeyondAssets;
use perlica_db::PlayerDb;
use perlica_proto::{CsHead, prost::Message};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use tracing::{debug, error, info, warn};

pub struct SessionContext {
    // This struct holds all the juicy context for a session. Think of it as the session's backpack, full of essential gear.
    pub assets: &'static BeyondAssets, // Game assets, like all the cool stuff in the game. Static means it's always there, chillin'.
    pub registry: &'static SessionRegistry, // The big list of all active sessions. Kinda like a VIP list, but for connections.
    pub db: &'static PlayerDb, // Our database connection. Where all the player data lives, safe and sound (hopefully!).
    pub addr: SocketAddr, // The address of the connected client. So we know who we're talking to, ya know?
}

pub async fn handle_connection(
    // This function is the bouncer for new connections. It sets up everything for a new player.
    socket: TcpStream,
    addr: SocketAddr,
    assets: &'static BeyondAssets,
    registry: &'static SessionRegistry,
    db: &'static PlayerDb,
) -> anyhow::Result<()> {
    let (reader, writer) = socket.into_split(); // Splitting the socket into a reader and a writer. Gotta read and write separately, multitasking FTW!

    let (outbound_tx, outbound_rx) = mpsc::channel::<Vec<u8>>(64); // Channel for sending data *out* to the client. Like a one-way street for packets.
    let (notify_tx, notify_rx) = mpsc::channel::<Notification>(32); // Channel for internal notifications. Server's gotta talk to itself sometimes, right? XD

    let handle = PlayerHandle::new(notify_tx); // A handle to notify this player. Like a pager, but for server events.

    let write_task = tokio::spawn(write_loop(writer, outbound_rx)); // Spawning a task to constantly send data. Keep those packets flowing!

    let ctx = SessionContext {
        assets,
        registry,
        db,
        addr,
    };

    let result = logic_loop(reader, outbound_tx, notify_rx, handle, ctx).await; // The main logic loop for the session. This is where the real game happens, fam.

    // outbound_tx dropped here, write_loop drains remaining frames then exits.
    let _ = write_task.await;

    result
}

// Drains the outbound channel and writes each pre-encoded frame to the socket.
// Exits when the sender side (logic loop) is dropped.
async fn write_loop(
    // This loop just writes data to the client. Simple, yet crucial.
    mut writer: tokio::net::tcp::OwnedWriteHalf,
    mut rx: mpsc::Receiver<Vec<u8>>,
) -> anyhow::Result<()> {
    while let Some(frame) = rx.recv().await {
        writer.write_all(&frame).await?;
    }
    Ok(())
}

async fn logic_loop(
    // The core logic for a player's session. It's like their personal game engine.
    mut reader: tokio::net::tcp::OwnedReadHalf,
    outbound_tx: mpsc::Sender<Vec<u8>>,
    mut notify_rx: mpsc::Receiver<Notification>,
    handle: PlayerHandle,
    ctx: SessionContext,
) -> anyhow::Result<()> {
    let mut player = Player::default(); // Creating a new player instance. Fresh spawn!
    let mut server_seq_id = 0u64;
    let mut registered = false;

    info!("session started"); // Session's live! Let's get this bread.

    let result = loop {
        tokio::select! { // This is where the magic happens: handling multiple async events at once. So efficient, much wow.
            result = read_packet(&mut reader) => { // Trying to read a packet from the client. What's up, client?
                match result {
                    Ok((cmd_id, body, client_seq_id)) => {
                        let mut net_ctx = NetContext::new(
                            &mut player,
                            ctx.db,
                            ctx.assets,
                            &outbound_tx,
                            client_seq_id,
                            &mut server_seq_id,
                        );
                        if let Err(e) = handle_command(&mut net_ctx, cmd_id, body).await { // Processing the command. Hope it's not a hack attempt, lol.
                            warn!(error = %e, cmd_id, "command error");
                        }

                        // uid is empty until on_login sets it.
                        if !registered && !player.uid.is_empty() { // If the player just logged in and isn't registered yet...
                            ctx.registry.register(player.uid.clone(), handle.clone()); // Registering the player. Welcome to the club!
                            info!(uid = %player.uid, online = ctx.registry.online(), "player online"); // Player's online! Let everyone know.
                            registered = true;
                        }
                    }
                    Err(e) if is_clean_disconnect(&e) => {
                        debug!("disconnected"); // Client disconnected. It's not you, it's them. Probably.
                        break Ok(());
                    }
                    Err(e) => break Err(e.into()),
                }
            }

            Some(notification) = notify_rx.recv() => {
                let mut net_ctx = NetContext::new(
                    &mut player,
                    ctx.db,
                    ctx.assets,
                    &outbound_tx,
                    0,
                    &mut server_seq_id,
                );
                handle_notification(&mut net_ctx, notification).await;
            }
        }
    };

    // Runs on every exit path, clean disconnect, error, or future break.
    if registered {
        // If the player was actually registered...
        if let Err(e) = ctx.db
        // Saving player data. Don't wanna lose all that hard-earned progress, right?
        {
            error!(uid = %player.uid, error = %e, "Save failed");
        }
        ctx.registry.unregister(&player.uid); // Unregistering the player. They're gone, but not forgotten (by the DB).
        info!(uid = %player.uid, online = ctx.registry.online(), "Player offline"); // Player's offline. Sadge.
    }

    result
}

// Reads one framed packet and returns (cmd_id, body, client_seq_id).
async fn read_packet(
    // This function reads a single packet from the client. It's like the mailman for game data.
    reader: &mut tokio::net::tcp::OwnedReadHalf,
) -> std::io::Result<(i32, Vec<u8>, u64)> {
    let head_size = reader.read_u8().await?;
    let body_size = reader.read_u16_le().await?;

    let mut head_buf = vec![0u8; head_size as usize];
    reader.read_exact(&mut head_buf).await?;

    let mut body_buf = vec![0u8; body_size as usize];
    reader.read_exact(&mut body_buf).await?;

    let head = CsHead::decode(&head_buf[..])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    Ok((head.msgid, body_buf, head.up_seqid))
}

fn is_clean_disconnect(e: &std::io::Error) -> bool {
    // Checking if the disconnect was graceful. No drama, please.
    matches!(
        e.kind(),
        std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
    )
}

// Dispatches an inbound server notification into the player's session.
// Each Notification variant maps to the logic that produces outbound frames.
async fn handle_notification(ctx: &mut NetContext<'_>, notification: Notification) {
    // Handling internal server notifications. The server's internal monologue, basically.
    match notification {
        #[allow(unreachable_patterns)]
        _ => {}
    }
}
