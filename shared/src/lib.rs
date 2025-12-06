use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub const SCOREBOARD_LENGTH: usize = 10;

/// Unique number per client for tracking the order of packets
pub type SequenceNumber = u64;
pub type TickNumber = u64;
pub type PlayerId = u64;
pub type BulletId = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalized(self) -> Self {
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
    pub id: PlayerId,
    pub name: String,
    pub position: Vec2,
    pub velocity: Vec2,
    pub health: i32,
    pub max_health: i32,
    pub score: u64,
    pub life: LifeState,
    pub last_processed_input: SequenceNumber, // For clearing input queue on client side
}

impl PlayerState {
    pub fn new(id: PlayerId, name: String, spawn_pos: Vec2, max_health: i32) -> Self {
        Self {
            id,
            name,
            position: spawn_pos,
            velocity: Vec2::new(0.0, 0.0),
            health: max_health,
            max_health,
            score: 0,
            life: LifeState::Alive,
            last_processed_input: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulletState {
    pub id: BulletId,
    pub owner_id: PlayerId,
    pub position: Vec2,
    pub velocity: Vec2,
    pub spawn_tick: TickNumber,
    pub damage: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInput {
    pub predicted_tick: TickNumber, // Client's predicted tick
    pub sequence: SequenceNumber,   // Monotonic input sequence number

    pub move_direction: Vec2,
    pub shoot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientPacket {
    Connect { player_name: String },
    Disconnect,
    Input(ClientInput),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerPacket {
    ConnectionAccepted {
        player_id: PlayerId,
        full_snapshot: Snapshot,
    },
    ConnectionRejected {
        reason: String,
    },
    Disconnect {
        reason: String,
    },
    FullSnapshot(Snapshot),
    DeltaSnapshot(DeltaSnapshot),
}

#[derive(Debug, Clone)]
pub struct ClientConnection {
    pub player_id: PlayerId,
    pub last_heard: Instant,
    pub latency: Duration, // Estimated round-trip time
}

impl ClientConnection {
    pub fn new(player_id: PlayerId) -> Self {
        Self {
            player_id,
            last_heard: Instant::now(),
            latency: Duration::from_millis(50), // Initial estimate
        }
    }

    pub fn is_timed_out(&self) -> bool {
        self.last_heard.elapsed() > CLIENT_TIMEOUT
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScoreEntry {
    pub player_id: PlayerId,
    pub score: u64,
}

impl Ord for ScoreEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.cmp(&other.score)
    }
}

impl PartialOrd for ScoreEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.score.partial_cmp(&other.score)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: TickNumber,

    pub players: Vec<PlayerState>,
    pub bullets: Vec<BulletState>,
    pub scoreboard: [ScoreEntry; SCOREBOARD_LENGTH],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaSnapshot {
    pub tick: TickNumber,
    pub base_tick: TickNumber, // Last acknowledged snapshot

    pub added_players: Vec<PlayerState>,
    pub removed_players: Vec<PlayerId>,
    pub updated_players: Vec<PlayerState>,

    pub added_bullets: Vec<BulletState>,
    pub removed_bullets: Vec<BulletId>,

    pub scoreboard: Option<[ScoreEntry; SCOREBOARD_LENGTH]>,
}
