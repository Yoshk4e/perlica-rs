CREATE TABLE beyond_destroyed_interactives (
    uid TEXT NOT NULL REFERENCES beyond_players(uid) ON DELETE CASCADE,
    scene_name TEXT NOT NULL,
    level_logic_id TEXT NOT NULL,
    PRIMARY KEY (uid, scene_name, level_logic_id)
);
