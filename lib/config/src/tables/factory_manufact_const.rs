use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FManufactConst {
    /// Per-recipe outcome stack cap inside the manufacture buffer.
    pub manufact_outcome_buffer_stack_max_count: u32,
    /// Maximum number of "sets" the player can queue at once.
    pub max_set_count: u32,
}
