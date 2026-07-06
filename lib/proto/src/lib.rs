#![allow(
    clippy::pedantic,
    clippy::needless_lifetimes,
    clippy::doc_markdown,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::module_inception,
    dead_code,
    unused_variables
)]

pub use prost;

include!("../out/_.rs");
include!("../out/net_message_impls.rs");
