//! Observer & statistics business logic.
//!
//! The observer handles 4 checkout types (relation board, power
//! connection map, outside resource, character work). Statistics
//! tracks production in bucket-based time windows
//! (`statisticBucketSteps=600`, `statisticBucketCount=60`).
//!
//! Three ops: `observer_op` (checkout data), `statistic_require`
//! (query stats), `statistic_set_bookmark_item_ids` (manage bookmarks).

use std::collections::HashMap;

use crate::factory::{FactoryComponent, FactoryManager};

impl FactoryManager {
    /// Process an observer checkout request. Returns the checkout type
    /// string so the handler can pick the right response payload.
    pub fn observer_checkout(
        &self,
        region_name: &str,
        _node_id: u32,
        _component_id: u32,
        op_type: &str,
    ) -> ObserverResult {
        let Some(region) = self.region(region_name) else {
            return ObserverResult::Error("region not found".into());
        };

        match op_type {
            "CheckoutRelationBoard" => {
                // Return the list of connections in this region.
                let connections: Vec<(u32, u32, i32)> = region
                    .connections
                    .iter()
                    .map(|c| (c.node_id_a, c.node_id_b, c.connection_type as i32))
                    .collect();
                ObserverResult::RelationBoard { connections }
            }
            "CheckoutPowerConnectionMap" => {
                // Return which nodes are powered + the power graph totals.
                let graph = crate::factory::power::PowerGraph::compute(region);
                ObserverResult::PowerConnectionMap {
                    powered_nodes: graph.powered_nodes.iter().copied().collect(),
                    total_generation: graph.total_generation,
                    total_consumption: graph.total_consumption,
                    total_storage: graph.total_storage,
                    total_stored: graph.total_stored,
                }
            }
            "CheckoutOutsideResource" => {
                // Return the resource nodes (miners/collectors) and their
                // current output.
                let mut nodes = vec![];
                for node in region.nodes.values() {
                    for (_, comp) in &node.components {
                        if let FactoryComponent::Collector(state) = comp {
                            let item_id = state
                                .items_round
                                .first()
                                .map(|s| s.item_id.clone())
                                .unwrap_or_default();
                            nodes.push((node.node_id, item_id, state.current_progress as i64));
                        }
                    }
                }
                ObserverResult::OutsideResource { nodes }
            }
            "CheckoutCharacterWork" => {
                // Return the character workers assigned to this region.
                let workers: Vec<(u32, u32, String)> = self
                    .character_work_state
                    .workers
                    .iter()
                    .filter(|w| w.region_name == region_name)
                    .map(|w| {
                        (
                            w.char_inst_id,
                            w.work_slot,
                            w.skill_ids.first().cloned().unwrap_or_default(),
                        )
                    })
                    .collect();
                ObserverResult::CharacterWork { workers }
            }
            _ => ObserverResult::Error(format!("unknown checkout type: {op_type}")),
        }
    }

    /// Query statistics. Returns production totals broken down by item.
    pub fn statistic_require(
        &self,
        region_name: &str,
        _rank_power: i32,
        _rank_productivity: i32,
        productivity_item_ids: &[String],
        all_productivity: bool,
    ) -> HashMap<String, u64> {
        let Some(region) = self.region(region_name) else {
            return HashMap::new();
        };

        if all_productivity {
            region.production_totals.clone()
        } else {
            region
                .production_totals
                .iter()
                .filter(|(id, _)| productivity_item_ids.contains(id))
                .map(|(k, v)| (k.clone(), *v))
                .collect()
        }
    }

    /// Set or remove bookmarked item IDs for statistics tracking.
    pub fn statistic_set_bookmark_item_ids(&mut self, item_ids: &[String], is_remove: bool) {
        // Bookmarks live on the SttState since it's the player-wide
        // factory state container. TODO: add a dedicated bookmarks
        // field once we add one to the struct.
        if is_remove {
            self.stt_state
                .visible_formulas
                .retain(|f| !item_ids.contains(f));
        } else {
            for id in item_ids {
                if !self.stt_state.visible_formulas.contains(id) {
                    self.stt_state.visible_formulas.push(id.clone());
                }
            }
        }
    }
}

/// Result of an observer checkout. The handler maps this to the
/// matching `ScdFactoryObserverPayloadRet*` proto.
#[derive(Debug, Clone)]
pub enum ObserverResult {
    RelationBoard {
        connections: Vec<(u32, u32, i32)>,
    },
    PowerConnectionMap {
        powered_nodes: Vec<u32>,
        total_generation: i64,
        total_consumption: i64,
        total_storage: i64,
        total_stored: i64,
    },
    OutsideResource {
        nodes: Vec<(u32, String, i64)>,
    },
    CharacterWork {
        workers: Vec<(u32, u32, String)>,
    },
    Error(String),
}
