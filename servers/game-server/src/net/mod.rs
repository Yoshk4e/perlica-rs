//! Networking layer for the game server.
//!
//! Custom binary protocol over TCP. Each packet is framed as:
//! `[head_size: u8][body_size: u16][head: CsHead][body: protobuf]`
//!
//! Multiple commands can be batched inside a `CsMergeMsg`; the router unpacks
//! them and dispatches each individually.
//!
//! - **context** — `NetContext` passed into every handler
//! - **session** — per-connection read/write loops and lifecycle
//! - **router**  — maps command IDs to handler functions
//! - **registry** — lets other systems look up a live session by UID
//! - **notify**  — server-push notifications outside the request cycle

pub mod context;
pub mod notify;
pub mod registry;
pub mod router;
pub mod session;

pub use context::NetContext;
#[allow(unused_imports)]
pub use notify::{Notification, PlayerHandle};
pub use registry::SessionRegistry;
pub use session::handle_connection;
