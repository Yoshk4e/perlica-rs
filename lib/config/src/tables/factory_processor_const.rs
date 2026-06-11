use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FProcessorConst {
    /// Seconds between two free refine-point refills (=14_400 = 4 hours).
    pub refine_point_recover_time: u64,
    /// Maximum number of refine points the processor can hold (=6).
    pub refine_point_max: u32,
    /// Minimum building level required to refine weapons.
    pub building_level_weapon_refine: u32,
    /// Minimum building level required to recast gems.
    pub building_level_gem_recast: u32,
}
