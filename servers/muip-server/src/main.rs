use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, head},
};
use common::logging::init_tracing;
use perlica_muip::{GmRequest, GmResponse};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{error, info};

static PANEL_HTML: &str = include_str!("../res/index.html");

#[derive(Debug, Clone, Deserialize)]
struct Config {
    #[serde(default)]
    muip: MuipConfig,
}

#[derive(Debug, Clone, Deserialize)]
struct MuipConfig {
    #[serde(default = "default_bind_host")]
    host: String,
    #[serde(default = "default_bind_port")]
    port: u16,
    #[serde(default = "default_token")]
    token: String,
    #[serde(default = "default_gm_host")]
    gm_host: String,
    #[serde(default = "default_gm_port")]
    gm_port: u16,
}

impl Default for MuipConfig {
    fn default() -> Self {
        Self {
            host: default_bind_host(),
            port: default_bind_port(),
            token: default_token(),
            gm_host: default_gm_host(),
            gm_port: default_gm_port(),
        }
    }
}

impl MuipConfig {
    fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    fn gm_addr(&self) -> String {
        format!("{}:{}", self.gm_host, self.gm_port)
    }
}

fn default_bind_host() -> String {
    "0.0.0.0".to_owned()
}
fn default_bind_port() -> u16 {
    8080
}
fn default_token() -> String {
    "1999".to_owned()
}
fn default_gm_host() -> String {
    "127.0.0.1".to_owned()
}
fn default_gm_port() -> u16 {
    2338
}

impl Config {
    fn load() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let path = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "Config.toml".to_owned());
        let contents = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&contents)?)
    }
}

#[derive(Clone)]
struct AppState {
    token: Arc<str>,
    gm_addr: Arc<str>,
}

#[derive(Debug, Deserialize)]
struct GmQuery {
    token: String,
    player_uid: Option<String>,
    command: String,
}

#[derive(Debug, Serialize)]
struct StatusPayload {
    online: usize,
    #[serde(rename = "playerCount")]
    player_count: usize,
    #[serde(rename = "maxPlayers")]
    max_players: usize,
    players: Vec<String>,
    status: &'static str,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    init_tracing(tracing::Level::DEBUG);

    let cfg = Config::load()?;
    let bind_addr: SocketAddr = cfg.muip.bind_addr().parse()?;

    let state = AppState {
        token: Arc::from(cfg.muip.token.as_str()),
        gm_addr: Arc::from(cfg.muip.gm_addr().as_str()),
    };

    let app = Router::new()
        .route("/", head(root_head).get(panel_handler))
        .route("/status", get(status_handler))
        .route("/status/server", get(status_handler))
        .route("/api/status", get(status_handler))
        .route("/api/players", get(players_handler))
        .route("/muip/gm", get(gm_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    info!("MUIP HTTP server listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn root_head() -> StatusCode {
    StatusCode::OK
}

async fn panel_handler() -> Html<&'static str> {
    Html(PANEL_HTML)
}

async fn status_handler(State(state): State<AppState>) -> impl IntoResponse {
    match send_gm(&state.gm_addr, GmRequest::Status).await {
        Ok(status) => (
            StatusCode::OK,
            Json(StatusPayload {
                online: status.online,
                player_count: status.online,
                max_players: 99999999,
                players: vec![],
                status: "online",
            }),
        )
            .into_response(),
        Err(e) => gm_bridge_error(e),
    }
}

async fn players_handler(State(state): State<AppState>) -> impl IntoResponse {
    match send_gm(&state.gm_addr, GmRequest::ListPlayers).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => gm_bridge_error(e),
    }
}

async fn gm_handler(
    State(state): State<AppState>,
    Query(query): Query<GmQuery>,
) -> impl IntoResponse {
    if query.token != *state.token {
        return (
            StatusCode::UNAUTHORIZED,
            Json(GmResponse::err(401, "invalid MUIP token")),
        )
            .into_response();
    }

    let request = match query.command.trim().to_ascii_lowercase().as_str() {
        "help" | "?" => {
            return (
                StatusCode::OK,
                Json(GmResponse::ok(
                    "commands: help, heal [all], level <n>, tp <scene> <x> <y> <z>, \
                     spawn <template> [x y z] [level], give weapon <template>, kick [reason]",
                )),
            )
                .into_response();
        }
        "info" | "list" | "players" | "listplayers" | "list_players" => GmRequest::ListPlayers,
        _ => {
            let Some(uid) = query.player_uid else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(GmResponse::err(
                        400,
                        "player_uid is required for this command",
                    )),
                )
                    .into_response();
            };
            GmRequest::Execute {
                player_uid: uid,
                command: query.command,
            }
        }
    };

    match send_gm(&state.gm_addr, request).await {
        Ok(response) => {
            let status = StatusCode::from_u16(response.retcode as u16)
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(response)).into_response()
        }
        Err(e) => {
            error!("GM forwarding failed: {e}");
            gm_bridge_error(e)
        }
    }
}

fn gm_bridge_error(e: impl std::fmt::Display) -> axum::response::Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(GmResponse::err(503, format!("GM bridge unavailable: {e}"))),
    )
        .into_response()
}

async fn send_gm(
    addr: &str,
    request: GmRequest,
) -> Result<GmResponse, Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = TcpStream::connect(addr).await?;

    let mut payload = serde_json::to_vec(&request)?;
    payload.push(b'\n');
    stream.write_all(&payload).await?;
    stream.flush().await?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err("empty GM response".into());
    }

    Ok(serde_json::from_str(trimmed)?)
}
