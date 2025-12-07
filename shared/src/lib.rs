use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    ops::{Add, Mul, Sub},
    time::{Duration, Instant, SystemTime},
};

// ===== World Constants ===== //
pub const WORLD_WIDTH: f32 = 10_000.0;
pub const WORLD_HEIGHT: f32 = 10_000.0;
pub const WORLD_PADDING: f32 = 20.0;

pub const WORLD_MIN_X: f32 = -WORLD_WIDTH / 2.0;
pub const WORLD_MAX_X: f32 = WORLD_WIDTH / 2.0;
pub const WORLD_MIN_Y: f32 = -WORLD_HEIGHT / 2.0;
pub const WORLD_MAX_Y: f32 = WORLD_HEIGHT / 2.0;

pub const WORLD_MIN_X_PADDED: f32 = WORLD_MIN_X + WORLD_PADDING;
pub const WORLD_MAX_X_PADDED: f32 = WORLD_MAX_X - WORLD_PADDING;
pub const WORLD_MIN_Y_PADDED: f32 = WORLD_MIN_Y + WORLD_PADDING;
pub const WORLD_MAX_Y_PADDED: f32 = WORLD_MAX_Y - WORLD_PADDING;

// ===== View Constants ===== //
pub const VIEW_WIDTH: f32 = 1920.0;
pub const VIEW_HEIGHT: f32 = 1080.0;
pub const VIEW_PADDING: f32 = 20.0;
pub const VIEW_SIZE: Vec2 = Vec2::new(
    VIEW_WIDTH + 2.0 * VIEW_PADDING,
    VIEW_HEIGHT + 2.0 * VIEW_PADDING,
);

// ===== Other Constants ===== //
pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);
pub const INTERPOLATION_DELAY: Duration = Duration::from_millis(100); // Client render delay
pub const LAG_COMPENSATION_HISTORY: Duration = Duration::from_secs(1);

pub const SCOREBOARD_LENGTH: usize = 10;

/// Unique number per client for tracking the order of packets
pub type SequenceNumber = u64;
pub type TickNumber = u64;
pub type PlayerId = u64;
pub type BulletId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
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

    pub fn dot(&self, rhs: &Vec2) -> f32 {
        self.x * rhs.x + self.y * rhs.y
    }

    pub fn inbound(&self, rhs: &Vec2, size: &Vec2) -> bool {
        (self.x - rhs.x).abs() <= size.x && (self.y - rhs.y).abs() <= size.y
    }
}

impl Add for Vec2 {
    type Output = Vec2;

    fn add(self, rhs: Self) -> Self::Output {
        Vec2 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Vec2;

    fn sub(self, rhs: Self) -> Self::Output {
        Vec2 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;

    fn mul(self, rhs: f32) -> Self::Output {
        Vec2 {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

// ===== States ===== //
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
pub struct UpdatedPlayerState {
    pub id: PlayerId,
    pub position: Vec2,
    pub velocity: Vec2,
    pub health: i32,
    pub score: u64,
    pub life: LifeState,
    pub last_processed_input: SequenceNumber, // For clearing input queue on client side
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

// ===== Client Input ===== //
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInput {
    pub prediected_time: SystemTime, // Client's predicted time
    pub sequence: SequenceNumber,    // Monotonic input sequence number

    pub move_direction: Vec2,
    pub shoot: bool,
}

// ===== Client & Server Packet ===== //
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientPacket {
    Connect { player_name: String },
    Disconnect,
    Input(ClientInput),
    TimeSync { client_send_time: SystemTime },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerPacket {
    ConnectionAccepted {
        player_id: PlayerId,
    },
    ConnectionRejected {
        reason: String,
    },
    Disconnect {
        reason: String,
    },
    ViewSnapshot(ViewSnapshot),
    TimeSync {
        client_send_time: SystemTime,
        server_recv_time: SystemTime,
        server_send_time: SystemTime,
    },
}

// ===== Client Connection ===== //
#[derive(Debug, Clone)]
pub struct ClientConnection {
    pub player_id: PlayerId,
    pub last_heard: Instant,
}

impl ClientConnection {
    pub fn new(player_id: PlayerId) -> Self {
        Self {
            player_id,
            last_heard: Instant::now(),
        }
    }

    pub fn is_timed_out(&self) -> bool {
        self.last_heard.elapsed() > CLIENT_TIMEOUT
    }
}

// ===== Score Entry (used in scoreboard) ===== //
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

// ===== View Snapshot ===== //
/// Player-specific snapshot, only visible entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewSnapshot {
    pub viewer_position: Vec2,
    pub tick: TickNumber,

    // TODO: maybe use Arc<[T]> instead of Vec
    pub players: Vec<PlayerState>,
    pub bullets: Vec<BulletState>,
    pub scoreboard: Vec<ScoreEntry>,
}

// ===== Server-side Lag Compensator ===== //
pub struct LagCompensator {
    snapshots: VecDeque<LagCompensatorSnapshot>,
}

// TODO: find a better name
pub struct LagCompensatorSnapshot {
    pub time: SystemTime,
    pub tick: TickNumber,
    pub player_positions: HashMap<PlayerId, Vec2>,
}

impl LagCompensator {
    pub fn new() -> Self {
        Self {
            snapshots: VecDeque::new(),
        }
    }

    /// Record current state of all players
    pub fn record_tick(&mut self, tick: TickNumber, players: &HashMap<PlayerId, PlayerState>) {
        let snapshot = LagCompensatorSnapshot {
            time: SystemTime::now(),
            tick,
            player_positions: players
                .iter()
                .filter_map(|(id, p)| match p.life {
                    LifeState::Alive => Some((*id, p.position)),
                    LifeState::Dead { respawn_time: _ } => None,
                })
                .collect(),
        };

        self.snapshots.push_back(snapshot);

        let cutoff = SystemTime::now() - LAG_COMPENSATION_HISTORY;
        while let Some(front) = self.snapshots.front() {
            if front.time < cutoff {
                self.snapshots.pop_front();
            } else {
                break;
            }
        }
    }

    /// Rewind all players to a specific time
    pub fn rewind_to_time(&self, target_time: SystemTime) -> HashMap<PlayerId, Vec2> {
        let idx = match self
            .snapshots
            .binary_search_by(|s| s.time.cmp(&target_time))
        {
            Ok(i) => return self.snapshots[i].player_positions.clone(),
            Err(i) => i,
        };

        // Interpolate between snapshots
        let prev = &self.snapshots[idx - 1];
        let next = &self.snapshots[idx];

        let total_duration = next.time.duration_since(prev.time).unwrap().as_secs_f32();
        let elapsed = target_time.duration_since(prev.time).unwrap().as_secs_f32();
        let t = (elapsed / total_duration).clamp(0.0, 1.0);

        // Interpolate all positions
        let mut result = HashMap::new();
        for (&player_id, &prev_pos) in &prev.player_positions {
            let interpolated_pos = match next.player_positions.get(&player_id) {
                Some(next_pos) => Vec2::new(
                    prev_pos.x + (next_pos.x - prev_pos.x) * t,
                    prev_pos.y + (next_pos.y - prev_pos.y) * t,
                ),
                None => prev_pos, // Player disconnected after prev snapshot
            };

            result.insert(player_id, interpolated_pos);
        }

        result
    }
}
