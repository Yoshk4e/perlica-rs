//! End-to-end interest scheduling tests driven through the real session loop.

use crate::net::registry::SessionRegistry;
use crate::net::session::handle_connection;
use config::BeyondAssets;
use perlica_db::{PlayerDb, PlayerRecordRef};
use perlica_logic::{
    bitset::BitsetManager,
    character::char_bag::CharBag,
    mail::MailManager,
    mission::{GuideManager, MissionManager},
    player::WorldState,
    scene::RevivalMode,
    wallet::WalletState,
};
use perlica_proto::{
    CsHead, CsLogin, CsMoveObjectMove, CsPing, CsSceneKillMonster, CsSceneLoadFinish, MotionInfo,
    MoveObjectMoveInfo, NetMessage, ScEnterSceneNotify, ScLogin, ScMoveObjectMove,
    ScObjectEnterView, ScObjectLeaveView, ScPing, ScSceneDestroyEntity, ScSelfSceneInfo, Vector,
    prost::Message,
};
use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{Instant, advance, sleep, timeout_at};

const SCENE: &str = "map01_lv001";
const SPAWN_X: f32 = 469.0;
const SPAWN_Y: f32 = 107.11;
const SPAWN_Z: f32 = 217.83;
const VIRTUAL_WATCHDOG: Duration = Duration::from_mins(5);
const DB_WATCHDOG: Duration = Duration::from_secs(15);
const ENEMY_BUDGET: usize = 6;
const INTERACTIVE_BUDGET: usize = 8;
const NPC_BUDGET: usize = 4;

// Keeps the runtime busy so the paused clock cannot auto-advance while a
// sqlx pool operation waits on its worker thread.
struct ClockFreeze(tokio::task::JoinHandle<()>);

impl ClockFreeze {
    fn new() -> Self {
        Self(tokio::spawn(async {
            loop {
                tokio::task::yield_now().await;
            }
        }))
    }
}

impl Drop for ClockFreeze {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn assets() -> &'static BeyondAssets {
    static ASSETS: OnceLock<&'static BeyondAssets> = OnceLock::new();
    ASSETS.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
        Box::leak(Box::new(BeyondAssets::load(path).expect("load assets")))
    })
}

fn registry() -> &'static SessionRegistry {
    static REGISTRY: OnceLock<&'static SessionRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| Box::leak(Box::new(SessionRegistry::new())))
}

async fn seed_player(tag: &str) -> (&'static PlayerDb, String) {
    let _freeze = ClockFreeze::new();
    let dir = std::env::temp_dir().join(format!("perlica-runtime-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let db = Box::leak(Box::new(PlayerDb::open(&dir).await.expect("open db")));

    let uid = format!("runtime-{tag}");
    let char_bag = CharBag::new(assets(), &[]).expect("char bag");
    let world = WorldState::default();
    let bitsets = BitsetManager::new();
    let missions = MissionManager::default();
    let guides = GuideManager::default();
    let mail = MailManager::new();
    let wallet = WalletState::default();
    db.save(
        &uid,
        PlayerRecordRef::from_parts(
            &char_bag,
            &world,
            &bitsets,
            None,
            RevivalMode::Default,
            &missions,
            &guides,
            &mail,
            &wallet,
        ),
    )
    .await
    .expect("seed player");

    (db, uid)
}

fn encode<T: NetMessage + Message>(message: &T, seq: u64) -> Vec<u8> {
    let head = CsHead {
        msgid: T::CMD_ID,
        up_seqid: seq,
        ..Default::default()
    }
    .encode_to_vec();
    let body = message.encode_to_vec();
    let mut bytes = Vec::with_capacity(3 + head.len() + body.len());
    bytes.push(u8::try_from(head.len()).expect("head size"));
    bytes.extend_from_slice(&u16::try_from(body.len()).expect("body size").to_le_bytes());
    bytes.extend_from_slice(&head);
    bytes.extend_from_slice(&body);
    bytes
}

fn take_frame(buf: &mut Vec<u8>) -> Option<(i32, Vec<u8>)> {
    if buf.len() < 3 {
        return None;
    }
    let head_size = buf[0] as usize;
    let body_size = usize::from(u16::from_le_bytes([buf[1], buf[2]]));
    assert!(head_size > 0, "empty protocol head");
    let total = 3 + head_size + body_size;
    if buf.len() < total {
        return None;
    }
    let head = CsHead::decode(&buf[3..3 + head_size]).expect("decode head");
    let body = buf[3 + head_size..total].to_vec();
    buf.drain(..total);
    Some((head.msgid, body))
}

#[derive(Default)]
struct EnterBatch {
    monsters: Vec<u64>,
    interactives: usize,
    npcs: usize,
}

fn enter_batch(body: &[u8]) -> EnterBatch {
    let view = ScObjectEnterView::decode(body).expect("decode enter view");
    let Some(detail) = view.detail else {
        return EnterBatch::default();
    };
    EnterBatch {
        monsters: detail
            .monster_list
            .iter()
            .filter_map(|m| m.common_info.as_ref().map(|c| c.id))
            .collect(),
        interactives: detail.interactive_list.len(),
        npcs: detail.npc_list.len(),
    }
}

fn move_packet(step: f32) -> CsMoveObjectMove {
    CsMoveObjectMove {
        move_info: vec![MoveObjectMoveInfo {
            objid: 1,
            motion_info: Some(MotionInfo {
                position: Some(Vector {
                    x: SPAWN_X + step,
                    y: SPAWN_Y,
                    z: SPAWN_Z + step,
                }),
                ..Default::default()
            }),
            server_time: 0,
        }],
    }
}

struct Harness {
    client: TcpStream,
    pending: Vec<u8>,
    seq: u64,
    _server: tokio::task::JoinHandle<()>,
}

impl Harness {
    async fn connect(tag: &str) -> (Self, String) {
        let (db, uid) = seed_player(tag).await;
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let server = tokio::spawn(async move {
            if let Ok((socket, _)) = listener.accept().await {
                let _ = handle_connection(socket, assets(), registry(), db).await;
            }
        });
        let client = TcpStream::connect(addr).await.expect("connect");
        (
            Self {
                client,
                pending: Vec::new(),
                seq: 0,
                _server: server,
            },
            uid,
        )
    }

    async fn send<T: NetMessage + Message>(&mut self, message: &T) {
        self.seq += 1;
        let frame = encode(message, self.seq);
        self.client.write_all(&frame).await.expect("write frame");
    }

    async fn send_raw(&mut self, bytes: &[u8]) {
        self.client.write_all(bytes).await.expect("write raw");
    }

    async fn login(&mut self, uid: &str) {
        self.send(&CsLogin {
            uid: uid.to_string(),
            ..Default::default()
        })
        .await;
        self.drain_until(ScLogin::CMD_ID).await;
        self.drain_until(ScEnterSceneNotify::CMD_ID).await;
    }

    async fn load_scene(&mut self) -> Vec<EnterBatch> {
        self.send(&CsSceneLoadFinish {
            scene_name: SCENE.to_string(),
        })
        .await;
        self.drain_until(ScSelfSceneInfo::CMD_ID).await
    }

    // Yields instead of parking so the paused clock stays frozen while the
    // server performs sqlx work on worker threads.
    async fn drain_until(&mut self, cmd_id: i32) -> Vec<EnterBatch> {
        let started = std::time::Instant::now();
        let mut enters = Vec::new();
        let mut seen = false;
        loop {
            for (id, body) in self.read_available() {
                if id == cmd_id {
                    seen = true;
                } else if id == ScObjectEnterView::CMD_ID {
                    enters.push(enter_batch(&body));
                }
            }
            if seen {
                return enters;
            }
            assert!(
                started.elapsed() < DB_WATCHDOG,
                "timed out waiting for cmd {cmd_id}"
            );
            tokio::task::yield_now().await;
        }
    }

    async fn pump_until(&mut self, cmd_id: i32) -> Vec<u8> {
        let mut enters = Vec::new();
        self.pump_until_collecting(cmd_id, &mut enters).await
    }

    async fn pump_until_collecting(
        &mut self,
        cmd_id: i32,
        enters: &mut Vec<EnterBatch>,
    ) -> Vec<u8> {
        let deadline = Instant::now() + VIRTUAL_WATCHDOG;
        loop {
            let (id, body) = timeout_at(deadline, self.next_frame())
                .await
                .unwrap_or_else(|_| panic!("timed out waiting for cmd {cmd_id}"));
            if id == cmd_id {
                return body;
            }
            if id == ScObjectEnterView::CMD_ID {
                enters.push(enter_batch(&body));
            }
        }
    }

    async fn next_frame(&mut self) -> (i32, Vec<u8>) {
        loop {
            if let Some(frame) = take_frame(&mut self.pending) {
                return frame;
            }
            let mut buf = [0u8; 8192];
            let n = self.client.read(&mut buf).await.expect("read frame");
            assert!(n > 0, "connection closed");
            self.pending.extend_from_slice(&buf[..n]);
        }
    }

    fn read_available(&mut self) -> Vec<(i32, Vec<u8>)> {
        let mut buf = [0u8; 8192];
        loop {
            match self.client.try_read(&mut buf) {
                Ok(0) => panic!("connection closed"),
                Ok(n) => self.pending.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => panic!("try_read failed: {e}"),
            }
        }
        let mut frames = Vec::new();
        while let Some(frame) = take_frame(&mut self.pending) {
            frames.push(frame);
        }
        frames
    }

    // Yields without awaiting a timer so the paused clock never advances.
    async fn drain_rounds(&mut self, rounds: usize) -> Vec<(i32, Vec<u8>)> {
        let mut frames = Vec::new();
        for _ in 0..rounds {
            frames.extend(self.read_available());
            tokio::task::yield_now().await;
        }
        frames
    }

    async fn drain_until_echoes(&mut self, expected: usize) -> Vec<(i32, Vec<u8>)> {
        let mut frames = Vec::new();
        for _ in 0..4096 {
            frames.extend(self.read_available());
            let echoes = frames
                .iter()
                .filter(|(id, _)| *id == ScMoveObjectMove::CMD_ID)
                .count();
            if echoes >= expected {
                break;
            }
            tokio::task::yield_now().await;
        }
        frames
    }
}

#[tokio::test(start_paused = true)]
async fn stationary_player_streams_monsters_over_zone_ticks() {
    let (mut h, uid) = Harness::connect("stream").await;
    h.login(&uid).await;
    let enters = h.load_scene().await;

    let mut unique: HashSet<u64> = HashSet::new();
    let mut monster_frames = 0usize;
    let mut leave_frames = 0usize;
    for batch in &enters {
        if !batch.monsters.is_empty() {
            monster_frames += 1;
        }
        unique.extend(batch.monsters.iter().copied());
    }

    let deadline = Instant::now() + VIRTUAL_WATCHDOG;
    while unique.len() <= 6 || monster_frames < 2 {
        let (id, body) = timeout_at(deadline, h.next_frame())
            .await
            .expect("interest frames timed out");
        if id == ScObjectEnterView::CMD_ID {
            let batch = enter_batch(&body);
            if !batch.monsters.is_empty() {
                monster_frames += 1;
            }
            unique.extend(batch.monsters.iter().copied());
        } else if id == ScObjectLeaveView::CMD_ID {
            leave_frames += 1;
        }
    }

    assert!(
        unique.len() > 6,
        "expected more than 6 unique monsters, got {}",
        unique.len()
    );
    assert!(
        monster_frames >= 2,
        "expected streaming across ticks, got {monster_frames} frames"
    );
    assert_eq!(
        leave_frames, 0,
        "a stationary player must not ghost entities out"
    );
}

#[tokio::test(start_paused = true)]
async fn movement_burst_does_not_trigger_interest_scans() {
    let (mut h, uid) = Harness::connect("burst").await;
    h.login(&uid).await;
    let enters = h.load_scene().await;
    let mut seen: HashSet<u64> = HashSet::new();
    for batch in &enters {
        seen.extend(batch.monsters.iter().copied());
    }
    h.drain_rounds(32).await;

    for step in 0..64 {
        h.send(&move_packet(step as f32 * 0.1)).await;
    }
    let frames = h.drain_until_echoes(64).await;
    let echoes = frames
        .iter()
        .filter(|(id, _)| *id == ScMoveObjectMove::CMD_ID)
        .count();
    let enters_during = frames
        .iter()
        .filter(|(id, _)| *id == ScObjectEnterView::CMD_ID)
        .count();
    assert_eq!(echoes, 64, "every move packet must be echoed");
    assert_eq!(
        enters_during, 0,
        "no scan may run while the clock is frozen"
    );

    advance(Duration::from_millis(50)).await;
    let frames = h.drain_rounds(64).await;
    let mut new_monsters = 0usize;
    for (id, body) in &frames {
        if *id == ScObjectEnterView::CMD_ID {
            for monster in enter_batch(body).monsters {
                if seen.insert(monster) {
                    new_monsters += 1;
                }
            }
        }
    }
    assert!(
        new_monsters >= 1,
        "streaming must resume on the next deadline"
    );
    assert!(
        new_monsters <= ENEMY_BUDGET,
        "a late wake admits one budget window, got {new_monsters}"
    );
}

#[tokio::test(start_paused = true)]
async fn split_ping_frame_survives_interest_ticks() {
    let (mut h, uid) = Harness::connect("ping").await;
    h.login(&uid).await;
    h.load_scene().await;
    h.drain_rounds(32).await;

    let marker = 0xDEAD_BEEF_u64;
    let frame = encode(&CsPing { client_ts: marker }, 7);
    let (head, rest) = frame.split_at(2);
    h.send_raw(head).await;
    sleep(Duration::from_millis(40)).await;
    h.send_raw(rest).await;

    let body = h.pump_until(ScPing::CMD_ID).await;
    let reply = ScPing::decode(&body[..]).expect("decode ScPing");
    assert_eq!(reply.client_ts, marker);

    let frames = h.drain_rounds(64).await;
    let duplicates = frames
        .iter()
        .filter(|(id, _)| *id == ScPing::CMD_ID)
        .count();
    assert_eq!(
        duplicates, 0,
        "a split frame must produce exactly one reply"
    );
}

#[tokio::test(start_paused = true)]
async fn killed_monster_readmits_once_after_cooldown() {
    let (mut h, uid) = Harness::connect("respawn").await;
    h.login(&uid).await;
    let enters = h.load_scene().await;
    h.drain_rounds(32).await;

    let target = enters
        .iter()
        .flat_map(|batch| batch.monsters.iter().copied())
        .next()
        .expect("initial pass must stream an enemy");

    h.send(&CsSceneKillMonster { id: target }).await;

    let body = h.pump_until(ScSceneDestroyEntity::CMD_ID).await;
    let destroy = ScSceneDestroyEntity::decode(&body[..]).expect("decode destroy");
    assert_eq!(destroy.id, target);

    advance(Duration::from_secs(61)).await;
    let frames = h.drain_rounds(128).await;
    let readmits = frames
        .iter()
        .filter(|(id, body)| {
            *id == ScObjectEnterView::CMD_ID && enter_batch(body).monsters.contains(&target)
        })
        .count();
    assert_eq!(readmits, 1, "cooldown expiry must readmit exactly once");

    advance(Duration::from_millis(500)).await;
    let frames = h.drain_rounds(64).await;
    let duplicates = frames
        .iter()
        .filter(|(id, body)| {
            *id == ScObjectEnterView::CMD_ID && enter_batch(body).monsters.contains(&target)
        })
        .count();
    assert_eq!(duplicates, 0, "no duplicate readmit after the cooldown");
}

#[tokio::test(start_paused = true)]
async fn late_wake_admits_single_budget_window() {
    let (mut h, uid) = Harness::connect("late").await;
    h.login(&uid).await;
    h.load_scene().await;
    h.drain_rounds(32).await;

    advance(Duration::from_millis(500)).await;
    let frames = h.drain_rounds(128).await;

    let mut enter_frames = 0usize;
    let mut monsters = 0usize;
    let mut interactives = 0usize;
    let mut npcs = 0usize;
    for (id, body) in &frames {
        if *id == ScObjectEnterView::CMD_ID {
            enter_frames += 1;
            let batch = enter_batch(body);
            monsters += batch.monsters.len();
            interactives += batch.interactives;
            npcs += batch.npcs;
        }
    }

    assert_eq!(
        enter_frames, 1,
        "one wake must emit exactly one enter view, got {enter_frames}"
    );
    assert!(
        (1..=ENEMY_BUDGET).contains(&monsters),
        "enemy budget violated: {monsters}"
    );
    assert!(
        interactives <= INTERACTIVE_BUDGET,
        "interactive budget violated: {interactives}"
    );
    assert!(npcs <= NPC_BUDGET, "npc budget violated: {npcs}");
}
