use crate::net::SessionRegistry;
use perlica_muip::{GmRequest, GmResponse};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

pub async fn run_gm_listener(
    addr: String,
    registry: &'static SessionRegistry,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(&addr).await?;
    info!("MUIP GM bridge listening on {}", listener.local_addr()?);

    loop {
        let (stream, peer) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, registry).await {
                warn!("MUIP GM connection {} failed: {}", peer, e);
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    registry: &'static SessionRegistry,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let trimmed = line.trim();

    if trimmed.is_empty() {
        return write_response(&mut writer, GmResponse::err(400, "empty GM request")).await;
    }

    let request: GmRequest = match serde_json::from_str(trimmed) {
        Ok(r) => r,
        Err(e) => {
            return write_response(
                &mut writer,
                GmResponse::err(400, format!("invalid GM request: {e}")),
            )
            .await;
        }
    };

    let response = match request {
        GmRequest::Status => GmResponse {
            retcode: 0,
            message: "ok".to_owned(),
            online: registry.online(),
            players: vec![],
        },
        GmRequest::ListPlayers => {
            let online = registry.online();
            GmResponse {
                retcode: 0,
                message: format!("{online} player(s) online"),
                online,
                players: registry.list_uids(),
            }
        }
        GmRequest::Execute {
            player_uid,
            command,
        } => {
            let Some(handle) = registry.get(&player_uid) else {
                return write_response(
                    &mut writer,
                    GmResponse::err(404, format!("player `{player_uid}` is not online")),
                )
                .await;
            };

            match handle.exec_muip(command).await {
                Some(result) => result.response,
                None => GmResponse::err(500, "player session stopped before command completed"),
            }
        }
    };

    write_response(&mut writer, response).await
}

async fn write_response(
    writer: &mut (impl AsyncWriteExt + Unpin),
    response: GmResponse,
) -> std::io::Result<()> {
    let mut payload = serde_json::to_vec(&response).map_err(std::io::Error::other)?;
    payload.push(b'\n');
    writer.write_all(&payload).await?;
    writer.flush().await
}
