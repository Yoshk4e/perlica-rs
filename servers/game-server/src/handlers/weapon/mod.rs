//! Weapon command handlers.

pub mod breakthrough;
pub mod equip;
pub mod exp;
pub mod gem;

pub use breakthrough::on_cs_weapon_breakthrough;
pub use equip::on_cs_weapon_puton;
pub use exp::on_cs_weapon_add_exp;
pub use gem::{on_cs_weapon_attach_gem, on_cs_weapon_detach_gem};
