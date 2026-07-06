#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(test, allow(clippy::unnecessary_cast))]

pub mod bitset;
pub mod character;
pub mod entity;
pub mod enums;
pub mod error;
pub mod factory;
pub mod interest;
pub mod item;
pub mod level_script;
pub mod mail;
pub mod mission;
pub mod movement;
pub mod player;
pub mod scene;
pub mod spatial;
pub mod traits;
pub mod wallet;

pub use error::{LogicError, Result};
