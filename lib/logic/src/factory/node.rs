//! `FactoryNode` along with appropriate geometry types.
//!
//! Clause 1 contains just the internal model. `to_proto()` (task 3.2) lives
//! in Clause 3, until then the handler generates `ScdFactorySyncNode`
//! directly. Here the expectation is that the proto builder would be able to
//! read those fields without any surprises, so the Clause 3 effort is purely
//! mechanical.

use std::collections::HashMap;

use super::GridPos;
use crate::enums::{FCComponentPos, FCDirection, FCMeshType, FCNodeType, FCPropertyKey};
use crate::factory::component::FactoryComponent;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A rect or line mesh on the grid. `points` is the corner list for `Rect`
/// (4 points, clockwise from top-left) or the endpoint list for `Line`.
#[derive(Debug, Clone)]
pub struct Mesh {
    pub mesh_type: FCMeshType,
    pub points: Vec<GridPos>,
}

/// A sub-port on a node, input or output slot for a conveyor / belt.
#[derive(Debug, Clone)]
pub struct SubPort {
    pub position: GridPos,
    pub direction: i32,
}

/// A binding between an interactive object and a node, employed by the client to bind together
/// scene `InteractiveObject` IDs with factory nodes (for instance, the `object_id: 1`
/// for the hub allows the client to send clicks to the correct building).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractiveObject {
    pub object_id: u32,
}

/// Placement and rendering data of a node.
///
/// `position` can be `None` only for the virtual node `Inventory` (node_id=1),
/// which exists out-of-map. `mesh` is `None` because of the same reason.
/// `world_*` fields are pre-calculated scene geometry and are non-zero only
/// for pre-placed nodes (TODO(Clause 3): always `None` currently as in
/// `bootstrap_region`; the handler-side proto builder holds hard-coded values
/// for `sp_hub_1` until there's a table source for them).
#[derive(Debug, Clone)]
pub struct NodeTransform {
    pub position: Option<GridPos>,
    pub direction: FCDirection,
    pub mesh: Option<Mesh>,
    pub scene_name: String,
    pub world_position: Option<Vector3>,
    pub world_rotation: Option<Vector3>,
    pub bc_port_in: Option<SubPort>,
    pub bc_port_out: Option<SubPort>,
}

#[derive(Debug, Clone)]
pub struct FactoryNode {
    pub node_id: u32,
    pub node_type: FCNodeType,
    pub template_id: String,
    pub transform: NodeTransform,
    pub is_deactive: bool,
    pub interactive_object: Option<InteractiveObject>,
    pub dynamic_property: HashMap<FCPropertyKey, String>,
    pub component_pos: HashMap<FCComponentPos, u32>,
    pub components: Vec<(u32, FactoryComponent)>,
}

impl FactoryNode {
    pub fn component(&self, id: u32) -> Option<&FactoryComponent> {
        self.components
            .iter()
            .find_map(|(cid, c)| (*cid == id).then_some(c))
    }

    pub fn component_mut(&mut self, id: u32) -> Option<&mut FactoryComponent> {
        self.components
            .iter_mut()
            .find_map(|(cid, c)| (*cid == id).then_some(c))
    }

    pub fn component_at(&self, pos: FCComponentPos) -> Option<&FactoryComponent> {
        let id = self.component_pos.get(&pos).copied()?;
        self.component(id)
    }

    pub fn component_at_mut(&mut self, pos: FCComponentPos) -> Option<&mut FactoryComponent> {
        let id = self.component_pos.get(&pos).copied()?;
        self.component_mut(id)
    }

    /// Serialize to the wire format. Converts transform, mesh, components,
    /// and dynamic properties into the matching `ScdFactorySyncNode`.
    pub fn to_proto(&self) -> perlica_proto::ScdFactorySyncNode {
        use perlica_proto::{
            ScdFactorySyncDynamicProperty, ScdFactorySyncInteractiveObject, ScdFactorySyncMesh,
            ScdFactorySyncNode, ScdFactorySyncTransform, ScdFactoryVector2Int,
        };

        let transform = Some(ScdFactorySyncTransform {
            position: self
                .transform
                .position
                .map(|p| ScdFactoryVector2Int { x: p.x, y: p.y }),
            direction: self.transform.direction as i32,
            mesh: self.transform.mesh.as_ref().map(|m| ScdFactorySyncMesh {
                mesh_type: m.mesh_type as i32,
                points: m
                    .points
                    .iter()
                    .map(|p| ScdFactoryVector2Int { x: p.x, y: p.y })
                    .collect(),
            }),
            scene_name: self.transform.scene_name.clone(),
            world_position: self
                .transform
                .world_position
                .map(|v| perlica_proto::Vector {
                    x: v.x as f32,
                    y: v.y as f32,
                    z: v.z as f32,
                }),
            world_rotation: self
                .transform
                .world_rotation
                .map(|v| perlica_proto::Vector {
                    x: v.x as f32,
                    y: v.y as f32,
                    z: v.z as f32,
                }),
            bc_port_in: None,
            bc_port_out: None,
        });

        let interactive_object =
            self.interactive_object
                .map(|io| ScdFactorySyncInteractiveObject {
                    object_id: io.object_id as u64,
                });

        let dynamic_property = Some(ScdFactorySyncDynamicProperty {
            values: self
                .dynamic_property
                .iter()
                .map(|(&k, v)| {
                    (
                        k as i32,
                        perlica_proto::ScdFactorySyncDynamicPropertyValue {
                            value: Some(
                                perlica_proto::scd_factory_sync_dynamic_property_value::Value::StringValue(
                                    v.clone(),
                                ),
                            ),
                        },
                    )
                })
                .collect(),
        });

        let component_pos = self
            .component_pos
            .iter()
            .map(|(&k, &v)| (k as i32, v))
            .collect();

        let components = self
            .components
            .iter()
            .map(|(id, comp)| comp.to_proto(*id))
            .collect();

        ScdFactorySyncNode {
            node_id: self.node_id,
            node_type: self.node_type as i32,
            template_id: self.template_id.clone(),
            transform,
            is_deactive: self.is_deactive,
            interactive_object,
            dynamic_property,
            component_pos,
            components,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::{FCComponentPos, FCDirection, FCNodeType};
    use crate::factory::component::{FactoryComponent, InventoryState};
    use std::collections::HashMap;

    fn empty_transform(scene: &str) -> NodeTransform {
        NodeTransform {
            position: None,
            direction: FCDirection::Up,
            mesh: None,
            scene_name: scene.to_string(),
            world_position: None,
            world_rotation: None,
            bc_port_in: None,
            bc_port_out: None,
        }
    }

    #[test]
    fn component_lookup_by_id_and_pos() {
        let mut node = FactoryNode {
            node_id: 2,
            node_type: FCNodeType::Hub,
            template_id: "sp_hub_1".to_string(),
            transform: empty_transform("map01_lv001"),
            is_deactive: false,
            interactive_object: None,
            dynamic_property: HashMap::new(),
            component_pos: HashMap::from([(FCComponentPos::Hub, 2u32)]),
            components: vec![(2, FactoryComponent::Hub)],
        };

        assert!(matches!(node.component(2), Some(FactoryComponent::Hub)));
        assert!(node.component(99).is_none());
        assert!(matches!(
            node.component_at(FCComponentPos::Hub),
            Some(FactoryComponent::Hub)
        ));

        // Mutate using position indexing - change the Hub for another
        // component just to confirm that `component_at_mut` really returns
        // a mutable reference which we can use for writing.
        if let Some(slot) = node.component_at_mut(FCComponentPos::Hub) {
            *slot = FactoryComponent::Inventory(InventoryState::default());
        }
        assert!(matches!(
            node.component_at(FCComponentPos::Hub),
            Some(FactoryComponent::Inventory(_))
        ));
    }
}
