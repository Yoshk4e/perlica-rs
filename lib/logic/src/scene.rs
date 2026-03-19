use crate::character::char_bag::{CharBag, CharIndex};
use crate::entity::{EntityKind, EntityManager, SceneEntity};
use crate::movement::MovementManager;
use config::BeyondAssets;
use perlica_proto::{
    LeaveObjectInfo, ScEnterSceneNotify, ScLeaveSceneNotify, ScObjectEnterView, ScObjectLeaveView,
    ScSceneCreateEntity, ScSceneDestroyEntity, ScSceneRevival, ScSceneTeleport, ScSelfSceneInfo,
    SceneCharacter, SceneImplEmpty, SceneMonster, SceneInteractive, SceneNpc, SceneObjectCommonInfo,
    SceneObjectDetailContainer, Vector, sc_self_scene_info::SceneImpl,
};

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfInfoReason {
    EnterScene = 0,
    ReviveDead = 1,
    ReviveRest = 2,
    ChangeTeam = 3,
    ReviveByItem = 4,
    ResetDungeon = 5,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityDestroyReason {
    Immediately = 0,
    Dead = 1,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum RevivalMode {
    #[default]
    Default = 0,
    RepatriatePoint = 1,
    CheckPoint = 2,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CheckpointInfo {
    pub scene_name: String,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneLoadingState {
    #[default]
    Idle,
    Loading,
    Active,
}

#[derive(Debug, Clone)]
pub struct SceneManager {
    pub current_scene: String,
    pub scene_id: u64,
    pub loading_state: SceneLoadingState,
    pub in_battle: bool,
    pub checkpoint: Option<CheckpointInfo>,
    pub current_revival_mode: RevivalMode,
}

impl Default for SceneManager {
    fn default() -> Self {
        Self {
            current_scene: "map01_lv001".to_string(),
            scene_id: 0,
            loading_state: SceneLoadingState::Idle,
            in_battle: false,
            checkpoint: None,
            current_revival_mode: RevivalMode::Default,
        }
    }
}

impl SceneManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_scene_transition(
        &mut self,
        new_scene: &str,
        position: Vector,
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) -> (ScEnterSceneNotify, ScLeaveSceneNotify) {
        entities.clear();

        let leave_notify = ScLeaveSceneNotify {
            role_id: 1, //TODO: figure out why and where is this even used
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
        };

        self.current_scene = new_scene.to_string();
        self.scene_id = assets.str_id_num.get_scene_id(new_scene).unwrap_or(0);
        self.loading_state = SceneLoadingState::Loading;

        let enter_notify = ScEnterSceneNotify {
            role_id: 1,
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
            position: Some(position),
        };
        (enter_notify, leave_notify)
    }

    pub fn finish_scene_load(
        &mut self,
        char_bag: &CharBag,
        movement: &MovementManager,
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) -> (ScObjectEnterView, ScSelfSceneInfo) {
        self.loading_state = SceneLoadingState::Active;

        // Update scene info
        self.scene_id = assets
            .str_id_num
            .get_scene_id(&self.current_scene)
            .unwrap_or(0);

        let char_list = self.pack_scene_chars(char_bag, movement);
        let monster_list = self.pack_scene_monsters(assets, entities);

        let enter_view = self.build_object_enter_view(char_list.clone(), monster_list.clone());
        let self_info =
            self.build_self_scene_info(SelfInfoReason::EnterScene, char_list, monster_list, vec![]);

        (enter_view, self_info)
    }

    pub fn build_object_enter_view(
        &self,
        char_list: Vec<SceneCharacter>,
        monster_list: Vec<SceneMonster>,
    ) -> ScObjectEnterView {
        ScObjectEnterView {
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
            detail: Some(SceneObjectDetailContainer {
                char_list,
                monster_list,
                interactive_list: vec![],
                npc_list: vec![],
                summon_list: vec![],
            }),
            has_extra_object: false,
        }
    }

    pub fn build_object_enter_view_full(
        &self,
        char_list: Vec<SceneCharacter>,
        monster_list: Vec<SceneMonster>,
        interactive_list: Vec<perlica_proto::SceneInteractive>,
        npc_list: Vec<SceneNpc>,
    ) -> ScObjectEnterView {
        ScObjectEnterView {
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
            detail: Some(SceneObjectDetailContainer {
                char_list,
                monster_list,
                interactive_list,
                npc_list,
                summon_list: vec![],
            }),
            has_extra_object: false,
        }
    }

    pub fn build_object_leave_view(&self, entity_ids: Vec<u64>) -> ScObjectLeaveView {
        let obj_list = entity_ids
            .into_iter()
            .map(|id| LeaveObjectInfo {
                obj_type: 0,
                obj_id: id,
            })
            .collect();

        ScObjectLeaveView {
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
            obj_list,
        }
    }

    pub fn build_self_scene_info(
        &self,
        reason: SelfInfoReason,
        char_list: Vec<SceneCharacter>,
        monster_list: Vec<SceneMonster>,
        revive_chars: Vec<u64>,
    ) -> ScSelfSceneInfo {
        ScSelfSceneInfo {
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
            detail: Some(SceneObjectDetailContainer {
                char_list,
                monster_list,
                interactive_list: vec![],
                npc_list: vec![],
                summon_list: vec![],
            }),
            last_camp_id: 0, //unused for now
            revive_chars,
            level_scripts: vec![],
            self_info_reason: reason as i32,
            unlock_area: vec![],
            revival_mode: self.current_revival_mode as i32,
            scene_impl: Some(SceneImpl::Empty(SceneImplEmpty {})), //since dungeons aren't implemented yet we'll default to empty for the time being
        }
    }

    // this should be called when you receive CS_SCENE_SET_REPATRIATE_POINT or change mode :)
    pub fn set_revival_mode(&mut self, mode: RevivalMode) {
        self.current_revival_mode = mode;
    }

    pub fn build_entity_destroy(
        &self,
        entity_id: u64,
        reason: EntityDestroyReason,
    ) -> ScSceneDestroyEntity {
        ScSceneDestroyEntity {
            scene_name: self.current_scene.clone(),
            id: entity_id,
            reason: reason as i32,
        }
    }

    pub fn build_entity_create(&self, entity_id: u64) -> ScSceneCreateEntity {
        ScSceneCreateEntity {
            scene_name: self.current_scene.clone(),
            id: entity_id,
        }
    }

    pub fn handle_revival(
        &mut self,
        char_bag: &mut CharBag,
        movement: &MovementManager,
        assets: &BeyondAssets,
        entities: &mut EntityManager,
        revival_mode: Option<RevivalMode>,
    ) -> (ScObjectEnterView, ScSelfSceneInfo, ScSceneRevival) {
        if let Some(mode) = revival_mode {
            self.set_revival_mode(mode);
        }
        // Find all dead characters in current team
        let team = &char_bag.teams[char_bag.meta.curr_team_index as usize];
        let revive_chars: Vec<u64> = team
            .char_team
            .iter()
            .filter_map(|slot| slot.char_index())
            .filter(|&idx| char_bag.chars[idx.as_usize()].is_dead)
            .map(|idx| idx.object_id())
            .collect();

        // Revive them (restore 50% HP)
        for &objid in &revive_chars {
            let idx = CharIndex::from_object_id(objid);
            if let Some(char) = char_bag.chars.get_mut(idx.as_usize()) {
                char.is_dead = false;
                char.hp = assets
                    .characters
                    .get_stats(&char.template_id, char.level, char.break_stage)
                    .map(|a| a.hp / 2.0)
                    .unwrap_or(50.0);
            }
        }

        // Pack scene objects
        let char_list = self.pack_scene_chars(char_bag, movement);
        let monster_list = self.pack_scene_monsters(assets, entities);

        // Build notifications
        let enter_view = self.build_object_enter_view(char_list.clone(), monster_list.clone());
        let self_info = self.build_self_scene_info(
            SelfInfoReason::ReviveDead,
            char_list,
            monster_list,
            revive_chars,
        );
        let revival = ScSceneRevival {};

        (enter_view, self_info, revival)
    }

    pub fn handle_active_team_update(
        &mut self,
        old_team_ids: &[u64],
        new_team_ids: &[u64],
        char_bag: &CharBag,
        movement: &MovementManager,
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) -> (
        Option<ScObjectLeaveView>,
        ScObjectEnterView,
        ScSelfSceneInfo,
    ) {
        let _ = assets;
        let new_set: std::collections::HashSet<u64> = new_team_ids.iter().copied().collect();
        let old_set: std::collections::HashSet<u64> = old_team_ids.iter().copied().collect();
        let leaving: Vec<u64> = old_team_ids
            .iter()
            .filter(|&&id| {
                if new_set.contains(&id) {
                    return false;
                }
                let idx = CharIndex::from_object_id(id);
                char_bag
                    .chars
                    .get(idx.as_usize())
                    .map(|c| !c.is_dead)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        let leave_view = if leaving.is_empty() {
            None
        } else {
            Some(self.build_object_leave_view(leaving))
        };

        let entering: Vec<u64> = new_team_ids
            .iter()
            .filter(|&&id| {
                if old_set.contains(&id) {
                    return false;
                }
                let idx = CharIndex::from_object_id(id);
                char_bag
                    .chars
                    .get(idx.as_usize())
                    .map(|c| !c.is_dead)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        let enter_view = self.build_object_enter_view(
            self.pack_scene_chars_for_ids(&entering, char_bag, movement),
            vec![],
        );
        let all_alive_ids: Vec<u64> = new_team_ids
            .iter()
            .filter(|&&id| {
                let idx = CharIndex::from_object_id(id);
                char_bag
                    .chars
                    .get(idx.as_usize())
                    .map(|c| !c.is_dead)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        let mut char_list = self.pack_scene_chars_for_ids(&all_alive_ids, char_bag, movement);

        let leader_id = char_bag.teams[char_bag.meta.curr_team_index as usize]
            .leader_index
            .object_id();
        if let Some(pos) = char_list
            .iter()
            .position(|c| c.common_info.as_ref().map(|ci| ci.id) == Some(leader_id))
        {
            if pos != 0 {
                let leader_char = char_list.remove(pos);
                char_list.insert(0, leader_char);
            }
        }

        let monster_list = self.pack_monsters_from_manager(entities, assets);

        let self_info =
            self.build_self_scene_info(SelfInfoReason::ChangeTeam, char_list, monster_list, vec![]);

        (leave_view, enter_view, self_info)
    }

    /// Builds a monster list from entities already tracked in the [`EntityManager`].
    ///
    /// Unlike [`pack_scene_monsters`], this method does **not** allocate new entity
    /// IDs or insert anything into the manager. It is used for team-change scene
    /// syncs where monsters are already present in the client scene and only need
    /// to be described, not freshly spawned.
    pub fn pack_monsters_from_manager(
        &self,
        entities: &EntityManager,
        assets: &BeyondAssets,
    ) -> Vec<SceneMonster> {
        use perlica_proto::SceneObjectCommonInfo;

        entities
            .monsters()
            .map(|e| {
                let level = assets
                    .enemy_spawns
                    .get(&self.current_scene)
                    .and_then(|spawns| spawns.iter().find(|s| s.template_id == e.template_id))
                    .map(|s| s.level as i32)
                    .unwrap_or(1);
					
				let origin_id = assets
                    .enemy_spawns
                    .get(&self.current_scene)
                    .and_then(|spawns| spawns.iter().find(|s| s.template_id == e.template_id))
                    .map(|s| s.origin_id as u64)
                    .unwrap_or(0);
					
                SceneMonster {
                    common_info: Some(SceneObjectCommonInfo {
                        id: e.id,
                        templateid: e.template_id.clone(),
                        position: Some(Vector {
                            x: e.pos_x,
                            y: e.pos_y,
                            z: e.pos_z,
                        }),
                        rotation: None,
                        belong_level_script_id: 0,
                        r#type: 16,
                    }),
                    origin_id,
                    level,
                }
            })
            .collect()
    }
    pub fn pack_scene_chars_for_ids(
        &self,
        char_ids: &[u64],
        char_bag: &CharBag,
        movement: &MovementManager,
    ) -> Vec<SceneCharacter> {
        let spawn_pos = Vector {
            x: movement.pos_x,
            y: movement.pos_y,
            z: movement.pos_z,
        };
        let spawn_rot = Vector {
            x: movement.rot_x,
            y: movement.rot_y,
            z: movement.rot_z,
        };

        char_ids
            .iter()
            .filter_map(|&objid| {
                let idx = CharIndex::from_object_id(objid);
                char_bag
                    .chars
                    .get(idx.as_usize())
                    .map(|char_data| SceneCharacter {
                        common_info: Some(SceneObjectCommonInfo {
                            id: objid,
                            templateid: char_data.template_id.clone(),
                            position: Some(spawn_pos),
                            rotation: Some(spawn_rot),
                            belong_level_script_id: 0,
                            r#type: 8,
                        }),
                        level: char_data.level,
                        name: "Player".to_string(),
                    })
            })
            .collect()
    }

    pub fn handle_team_index_switch(
        &mut self,
        old_team_ids: &[u64],
        new_team_ids: &[u64],
        char_bag: &CharBag,
        movement: &MovementManager,
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) -> (
        Option<ScObjectLeaveView>,
        ScObjectEnterView,
        ScSelfSceneInfo,
    ) {
        self.handle_active_team_update(
            old_team_ids,
            new_team_ids,
            char_bag,
            movement,
            assets,
            entities,
        )
    }

    pub fn handle_inactive_team_update(
        &self,
        new_team_ids: &[u64],
        char_bag: &CharBag,
        movement: &MovementManager,
        assets: &BeyondAssets,
        entities: &EntityManager,
    ) -> ScSelfSceneInfo {
        let alive_ids: Vec<u64> = new_team_ids
            .iter()
            .filter(|&&id| {
                let idx = CharIndex::from_object_id(id);
                char_bag
                    .chars
                    .get(idx.as_usize())
                    .map(|c| !c.is_dead)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        let mut char_list = self.pack_scene_chars_for_ids(&alive_ids, char_bag, movement);
        let monster_list = self.pack_monsters_from_manager(entities, assets);

        // Put leader first if it's part of this new_team_ids.
        let leader_id = char_bag.teams[char_bag.meta.curr_team_index as usize]
            .leader_index
            .object_id();
        if let Some(pos) = char_list
            .iter()
            .position(|c| c.common_info.as_ref().map(|ci| ci.id) == Some(leader_id))
        {
            if pos != 0 {
                let leader_char = char_list.remove(pos);
                char_list.insert(0, leader_char);
            }
        }

        self.build_self_scene_info(SelfInfoReason::ChangeTeam, char_list, monster_list, vec![])
    }

    pub fn build_teleport(
        &self,
        obj_id_list: Vec<u64>,
        position: Vector,
        rotation: Option<Vector>,
        server_time: u32,
        teleport_reason: i32,
    ) -> ScSceneTeleport {
        ScSceneTeleport {
            obj_id_list,
            scene_name: self.current_scene.clone(),
            position: Some(position),
            rotation,
            server_time,
            teleport_reason,
        }
    }

    pub fn set_battle_mode(&mut self, in_battle: bool) {
        self.in_battle = in_battle;
    }

    pub fn pack_scene_chars(
        &self,
        char_bag: &CharBag,
        movement: &MovementManager,
    ) -> Vec<SceneCharacter> {
        let team = &char_bag.teams[char_bag.meta.curr_team_index as usize];

        let spawn_pos = Vector {
            x: movement.pos_x,
            y: movement.pos_y,
            z: movement.pos_z,
        };
        let spawn_rot = Vector {
            x: movement.rot_x,
            y: movement.rot_y,
            z: movement.rot_z,
        };

        // Collect in original order
        let mut chars: Vec<SceneCharacter> = team
            .char_team
            .iter()
            .filter_map(|slot| slot.char_index())
            .map(|idx| {
                let char_data = &char_bag.chars[idx.as_usize()];
                SceneCharacter {
                    common_info: Some(SceneObjectCommonInfo {
                        id: idx.object_id(),
                        templateid: char_data.template_id.clone(),
                        position: Some(spawn_pos),
                        rotation: Some(spawn_rot),
                        belong_level_script_id: 0,
                        r#type: 8,
                    }),
                    level: char_data.level,
                    name: "Player".to_string(),
                }
            })
            .collect();

        // Move leader to the front if present
        let leader_id = team.leader_index.object_id();
        if let Some(pos) = chars
            .iter()
            .position(|c| c.common_info.as_ref().map(|ci| ci.id) == Some(leader_id))
        {
            if pos != 0 {
                let leader_char = chars.remove(pos);
                chars.insert(0, leader_char);
            }
        }

        chars
    }

    pub fn pack_scene_monsters(
        &self,
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) -> Vec<SceneMonster> {
        let Some(spawns) = assets.enemy_spawns.get(&self.current_scene) else {
            return vec![];
        };

        spawns
            .iter()
            .map(|enemy| {
                let id = entities.next_monster_id();
                entities.insert(SceneEntity {
                    id,
                    template_id: enemy.template_id.clone(),
                    kind: EntityKind::Enemy,
                    pos_x: enemy.position.x,
                    pos_y: enemy.position.y,
                    pos_z: enemy.position.z,
                });

                SceneMonster {
                    common_info: Some(SceneObjectCommonInfo {
                        id,
                        templateid: enemy.template_id.clone(),
                        position: Some(Vector {
                            x: enemy.position.x,
                            y: enemy.position.y,
                            z: enemy.position.z,
                        }),
                        rotation: Some(Vector {
                            x: enemy.rotation.x,
                            y: enemy.rotation.y,
                            z: enemy.rotation.z,
                        }),
                        belong_level_script_id: 0, //TODO: load from asset
                        r#type: 16,
                    }),
                    origin_id: enemy.origin_id as u64, // entity logic id
                    level: enemy.level as i32,
                }
            })
            .collect()
    }

    pub fn pack_single_monster(&self, entity: &SceneEntity, level: i32, origin_id: u64) -> SceneMonster {
        SceneMonster {
            common_info: Some(SceneObjectCommonInfo {
                id: entity.id,
                templateid: entity.template_id.clone(),
                position: Some(Vector {
                    x: entity.pos_x,
                    y: entity.pos_y,
                    z: entity.pos_z,
                }),
                rotation: None,
                belong_level_script_id: 0,
                r#type: 16,
            }),
            origin_id,
            level,
        }
    }

    // Pack a single character for dynamic spawning (e.g., multiplayer peer(wink, wink))
    pub fn pack_single_char(
        &self,
        objid: u64,
        template_id: String,
        level: i32,
        position: Vector,
        rotation: Vector,
    ) -> SceneCharacter {
        SceneCharacter {
            common_info: Some(SceneObjectCommonInfo {
                id: objid,
                templateid: template_id,
                position: Some(position),
                rotation: Some(rotation),
                belong_level_script_id: 0,
                r#type: 8,
            }),
            level,
            name: "Player".to_string(),
        }
    }

    pub fn scene_name(&self) -> &str {
        &self.current_scene
    }

    pub fn is_in_scene(&self) -> bool {
        self.loading_state == SceneLoadingState::Active
    }

    pub fn set_checkpoint(&mut self, checkpoint: CheckpointInfo) {
        self.checkpoint = Some(checkpoint);
    }

    pub fn get_checkpoint(&self) -> Option<&CheckpointInfo> {
        self.checkpoint.as_ref()
    }

    // Update scene from world state (call on login/restore)
    pub fn update_from_world(&mut self, world: &crate::player::WorldState, assets: &BeyondAssets) {
        self.current_scene = world.last_scene.clone();
        self.scene_id = assets
            .str_id_num
            .get_scene_id(&world.last_scene)
            .unwrap_or(0);
    }
}
