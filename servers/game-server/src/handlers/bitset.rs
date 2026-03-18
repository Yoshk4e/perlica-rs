use crate::net::NetContext;
use perlica_logic::bitset::BitsetType;
use perlica_proto::{
    BitsetData, CsBitsetAdd, CsBitsetRemove, ScBitsetAdd, ScBitsetRemove, ScSyncAllBitset,
};
use tracing::{debug, info, warn};

/// Sets one or more bits in a named bitset.
///
/// Bitsets are used to track boolean flags across many systems (items found,
/// areas visited, wiki entries read, etc.). Unknown type IDs are silently
/// skipped with a warning rather than disconnecting the client.
pub async fn on_cs_bitset_add(ctx: &mut NetContext<'_>, req: CsBitsetAdd) -> ScBitsetAdd {
    let type_name = BitsetType::from_i32(req.r#type)
        .map(|t| format!("{:?}", t))
        .unwrap_or_else(|| "Unknown".to_string());

    debug!(
        "bitset add request: type={}, bits={:?}",
        type_name, req.value
    );

    for &bit in &req.value {
        if let Some(bitset_type) = BitsetType::from_i32(req.r#type) {
            ctx.player.bitsets.set(bitset_type, bit);
            debug!("bit added: type={}, bit={}", type_name, bit);
        } else {
            warn!("unknown bitset type: type_id={}, bit={}", req.r#type, bit);
        }
    }

    info!(
        "bits added successfully: type={}, count={}",
        type_name,
        req.value.len()
    );

    ScBitsetAdd {
        r#type: req.r#type,
        value: req.value.clone(),
        source: 0,
    }
}

/// Clears one or more bits in a named bitset.
///
/// Only bits that were previously set are affected; clearing an already-clear
/// bit is a no-op. Unknown type IDs are silently skipped.
pub async fn on_cs_bitset_remove(ctx: &mut NetContext<'_>, req: CsBitsetRemove) -> ScBitsetRemove {
    let type_name = BitsetType::from_i32(req.r#type)
        .map(|t| format!("{:?}", t))
        .unwrap_or_else(|| "Unknown".to_string());

    debug!(
        "bitset remove request: type={}, bits={:?}",
        type_name, req.value
    );

    for &bit in &req.value {
        if let Some(bitset_type) = BitsetType::from_i32(req.r#type) {
            ctx.player.bitsets.unset(bitset_type, bit);
            debug!("bit removed: type={}, bit={}", type_name, bit);
        }
    }

    info!(
        "bits removed successfully: type={}, count={}",
        type_name,
        req.value.len()
    );

    ScBitsetRemove {
        r#type: req.r#type,
        value: req.value.clone(),
        source: 0,
    }
}

/// Pushes the full bitset state as `ScSyncAllBitset`.
///
/// Iterates over all known [`BitsetType`] values and bundles their current bit
/// sets into a single notification. Called once during the login sequence.
///
/// Returns `false` if the send channel is closed.
pub async fn push_bitsets(ctx: &mut NetContext<'_>) -> bool {
    let bitset: Vec<BitsetData> = (1..20)
        .map(|t| {
            let bits = BitsetType::from_i32(t)
                .map(|bitset_type| ctx.player.bitsets.get_bits(bitset_type))
                .unwrap_or_default();

            BitsetData {
                r#type: t,
                value: bits.into_iter().map(|b| b as u64).collect(),
            }
        })
        .collect();

    debug!(
        "pushing bitsets: uid={}, count={}",
        ctx.player.uid,
        bitset.len()
    );

    ctx.notify(ScSyncAllBitset { bitset }).await.is_ok()
}
