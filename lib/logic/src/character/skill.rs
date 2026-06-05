//! Character skill logic: max-level lookups and skill validation helpers.

use config::BeyondAssets;

/// Returns the maximum level a specific skill can reach for a given character template.
///
/// Searches the character's skill bundles for one containing `skill_id`,
/// then returns the highest level among all entries in that bundle.
/// Falls back to 1 if the skill is not found in the asset tables.
pub fn max_skill_level(template_id: &str, skill_id: &str, assets: &BeyondAssets) -> u32 {
    assets
        .char_skills
        .get_char_skills(template_id)
        .iter()
        .find(|b| b.entries.iter().any(|e| e.skill_id == skill_id))
        .and_then(|b| b.entries.iter().map(|e| e.level).max())
        .unwrap_or(1)
}
