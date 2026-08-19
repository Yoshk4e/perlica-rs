use crate::net::NetContext;
use perlica_logic::item::BagTransfer;
use perlica_proto::{
    CsItemBagBagToFactoryDepot, CsItemBagFactoryDepotToBag, CsItemBagFactoryDepotToBagGrid,
    ScItemBagBagToFactoryDepot, ScItemBagSyncModify,
};
use std::collections::HashMap;
use tracing::warn;

/// Push the `SC_ITEM_BAG_SYNC_MODIFY` payload produced by a bag <-> depot
/// transfer. The client's `_HandleInventoryModify` applies the `bag` grids /
/// `delList` against its item bag and the `factoryDepot` item deltas against
/// its factory depot in a single pass.
async fn push_transfer(ctx: &mut NetContext<'_>, transfer: BagTransfer) {
    let _ = ctx
        .notify(ScItemBagSyncModify {
            depot: HashMap::new(),
            bag: Some(transfer.bag),
            factory_depot: Some(transfer.factory_depot),
            cannot_destroy: HashMap::new(),
            use_blackboard: None,
            is_new: false,
        })
        .await;
}

async fn persist(ctx: &mut NetContext<'_>) {
    if let Err(e) = ctx
        .db
        .persist_char_bag_incremental(&ctx.player.uid, &mut ctx.player.char_bag)
        .await
    {
        warn!(
            "Failed to persist char_bag after item-bag transfer: uid={}, error={}",
            ctx.player.uid, e
        );
    }
}

/// `CS_ITEM_BAG_FACTORY_DEPOT_TO_BAG_GRID` (cmd 69): move a stackable from the
/// factory depot into a specific bag grid slot. The client has no dedicated
/// reply message for this op; it reads the pushed `SC_ITEM_BAG_SYNC_MODIFY`.
pub async fn on_cs_item_bag_factory_depot_to_bag_grid(
    ctx: &mut NetContext<'_>,
    req: CsItemBagFactoryDepotToBagGrid,
) {
    if req.inst_id != 0 {
        warn!(
            "FactoryDepotToBagGrid: instanced factory item {inst_id} of {id} not supported (factory depot holds stackables only)",
            inst_id = req.inst_id,
            id = req.id
        );
        return;
    }
    match ctx
        .player
        .char_bag
        .item_manager
        .move_factory_depot_to_bag_grid(&req.id, req.count, req.grid_index)
    {
        Ok(transfer) => {
            push_transfer(ctx, transfer).await;
            persist(ctx).await;
        }
        Err(e) => {
            warn!(
                "FactoryDepotToBagGrid failed: uid={}, id={}, count={}, grid={}, err={:?}",
                ctx.player.uid, req.id, req.count, req.grid_index, e
            );
        }
    }
}

/// `CS_ITEM_BAG_FACTORY_DEPOT_TO_BAG` (cmd 64): move a batch of stackables
/// from the factory depot into the first empty bag slots.
pub async fn on_cs_item_bag_factory_depot_to_bag(
    ctx: &mut NetContext<'_>,
    req: CsItemBagFactoryDepotToBag,
) {
    match ctx
        .player
        .char_bag
        .item_manager
        .move_factory_depot_to_bag(&req.items)
    {
        Ok(transfer) => {
            push_transfer(ctx, transfer).await;
            persist(ctx).await;
        }
        Err(e) => {
            warn!(
                "FactoryDepotToBag failed: uid={}, err={:?}",
                ctx.player.uid, e
            );
        }
    }
}

/// `CS_ITEM_BAG_BAG_TO_FACTORY_DEPOT` (cmd 65): move the items in the given
/// bag grid slots back into the factory depot. The client expects a direct
/// `SC_ITEM_BAG_BAG_TO_FACTORY_DEPOT` reply (`notAllSuccess=false` on full
/// success) plus the `SC_ITEM_BAG_SYNC_MODIFY` that applies the change.
pub async fn on_cs_item_bag_bag_to_factory_depot(
    ctx: &mut NetContext<'_>,
    req: CsItemBagBagToFactoryDepot,
) -> ScItemBagBagToFactoryDepot {
    let all_success = match ctx
        .player
        .char_bag
        .item_manager
        .move_bag_to_factory_depot(&req.grid_list)
    {
        Ok(transfer) => {
            push_transfer(ctx, transfer).await;
            persist(ctx).await;
            true
        }
        Err(e) => {
            warn!(
                "BagToFactoryDepot failed: uid={}, grids={:?}, err={:?}",
                ctx.player.uid, req.grid_list, e
            );
            false
        }
    };
    ScItemBagBagToFactoryDepot {
        not_all_success: !all_success,
    }
}
