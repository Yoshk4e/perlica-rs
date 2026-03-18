//! Network module for the game server.
//!
//! This module provides the networking layer for handling client connections
//! and routing game commands. It implements a custom binary protocol over TCP.
//!
//! # Architecture Overview
//!
//! ```text
//! +-----------------+     +----------------+     +------------------+
//! |     Client      | --> |  TcpListener   | --> |  handle_connection
//! +-----------------+     +----------------+     +------------------+
//!                                                        |
//!                        +-------------------------------+
//!                        |
//!                        v
//!                 +----------------+
//!                 |  logic_loop    | <--- Main game loop per player
//!                 +----------------+
//!                        |
//!         +--------------+--------------+
//!         |              |              |
//!         v              v              v
//!    read_packet   notify_rx      handle_command
//!         |              |              |
//!         v              |              v
//!    handle_command     |       handlers::*
//!         |              |
//!         v              v
//!    NetContext    handle_notification
//!         |
//!         v
//!    send() / notify()
//!         |
//!         v
//!    write_loop --> Client
//! ```
//!
//! # Protocol
//!
//! ## Packet Format
//!
//! Each packet uses a simple framing format:
//! ```text
//! [head_size: u8][body_size: u16][head: bytes][body: bytes]
//! ```
//!
//! - `head_size`: Size of the header in bytes (max 255)
//! - `body_size`: Size of the body in bytes (max 65535)
//! - `head`: Protobuf-encoded `CsHead` containing `msgid` and `up_seqid`
//! - `body`: Protobuf-encoded message body specific to `msgid`
//!
//! ## Merge Packets
//!
//! Multiple commands can be bundled into a single `CsMergeMsg` packet
//! for efficiency. The router unpacks these and dispatches each command
//! individually.
//!
//! # Modules
//!
//! - **context**: `NetContext` struct for request handling
//! - **registry**: Session management for player lookup by UID
//! - **router**: Command routing and handler dispatch
//! - **session**: Connection handling and lifecycle management
//! - **notify**: Notification system for server-initiated messages

pub mod context;
pub mod notify;
pub mod registry;
pub mod router;
pub mod session;

// Re-export commonly used types for convenience
pub use context::NetContext;
#[allow(unused_imports)]
pub use notify::{Notification, PlayerHandle};
pub use registry::SessionRegistry;
pub use session::handle_connection;
