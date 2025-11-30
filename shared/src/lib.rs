use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const SCOREBOARD_LENGTH: usize = 10;

/// Unique number per client for tracking the order of packets
pub type SequenceNumber = u64;
pub type TickNumber = u64;
pub type PlayerId = u64;
pub type BulletId = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    x: f32,
    y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        Self {
            x: self.x / len,
            y: self.y / len,
        }
    }
}

// TODO: add Spawning state where player is invincible for a certain amount of time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LifeState {
    Alive,
    Dead { respawn_time: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    id: PlayerId,
    name: String,
    position: Vec2,
    velocity: Vec2,
    health: i32,
    max_health: i32,
    score: u64,
    life: LifeState,
}

impl PlayerState {
    fn new(id: PlayerId, name: String, spawn_pos: Vec2, max_health: i32) -> Self {
        Self {
            id,
            name,
            position: spawn_pos,
            velocity: Vec2::new(0.0, 0.0),
            health: max_health,
            max_health,
            score: 0,
            life: LifeState::Alive,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulletState {
    id: BulletId,
    owner_id: PlayerId,
    position: Vec2,
    velocity: Vec2,
    damage: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInput {
    tick: TickNumber,         // Client's predicted tick
    sequence: SequenceNumber, // Monotonic input sequence number

    move_direction: Vec2,
    shoot: bool,
}

#[derive(Debug, Clone)]
pub struct ClientConnection {
    player_id: PlayerId,
    last_heard: Instant,
    last_snapshot_sent: Instant,
    last_acknowledged_tick: TickNumber,
    latency: Duration, // Estimated round-trip time
}

impl ClientConnection {
    pub fn new(player_id: PlayerId, current_tick: TickNumber) -> Self {
        let instant_now = Instant::now();

        Self {
            player_id,
            last_heard: instant_now,
            last_snapshot_sent: instant_now,
            last_acknowledged_tick: current_tick,
            latency: Duration::ZERO,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientPacket {
    Connect { player_name: String },
    Disconnect,
    Input(ClientInput),
    AcknowledgeSnapshot { tick: TickNumber },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerPacket {
    ConnectionAccepted { player_id: PlayerId },
    ConnectionRejected { reason: String },
    Disconnect { reason: String },
    FullSnapshot(Snapshot),
    DeltaSnapshot(DeltaSnapshot),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreEntry {
    player_id: PlayerId,
    score: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    tick: TickNumber,

    players: Vec<PlayerState>,
    bullets: Vec<BulletState>,
    scoreboard: [ScoreEntry; SCOREBOARD_LENGTH],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaSnapshot {
    tick: TickNumber,
    base_tick: TickNumber, // Last acknowledged snapshot

    added_players: Vec<PlayerState>,
    removed_players: Vec<PlayerId>,
    updated_players: Vec<PlayerState>,

    added_bullets: Vec<BulletState>,
    removed_bullets: Vec<BulletId>,

    scoreboard: Option<[ScoreEntry; SCOREBOARD_LENGTH]>,
}
