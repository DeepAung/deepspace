// TODO: remove all unwraps

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    ops::{Add, Mul, Sub},
    time::{Duration, Instant, SystemTime},
};

// ===== Default Constants ===== //
pub const TICK_RATE: u32 = 60; // Server simulation rate (Hz)
pub const TICK_DURATION: Duration = Duration::from_nanos(1_000_000_000 / TICK_RATE as u64);

pub const SNAPSHOT_RATE: u32 = 20; // Snapshots per second to each client
pub const SNAPSHOT_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / SNAPSHOT_RATE as u64);

pub const TIME_SYNC_DURATION: Duration = Duration::from_secs(1);

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

// ===== Player Constants ===== //
pub const PLAYER_MOVE_SPEED: f32 = 300.0;
pub const PLAYER_MAX_HEALTH: i32 = 100;
pub const PLAYER_RESPAWN_TIME_SECS: f32 = 5.0;
pub const MAX_PLAYERS: usize = 1_000;
pub const PLAYER_RADIUS: f32 = 20.0;

// ===== Bullet Constants ===== //
pub const BULLET_LIFETIME: Duration = Duration::from_secs(3); // 3 seconds
pub const BULLET_LIFETIME_TICKS: TickNumber = BULLET_LIFETIME.as_secs() * (TICK_RATE as u64);
pub const BULLET_SPEED: f32 = 800.0;
pub const BULLET_INSTANT_HIT_MAX_RANGE: f32 = 20.0;
pub const BULLET_DAMAGE: i32 = 20;
pub const MAX_BULLETS: usize = 100_000;
pub const BULLET_RADIUS: f32 = 5.0;

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
pub const INTERPOLATION_DELAY: Duration = Duration::from_millis(50); // Client render delay
pub const LAG_COMPENSATION_HISTORY: Duration = Duration::from_secs(5);
pub const MAX_PACKET_SIZE: usize = 65535;

pub const SCOREBOARD_LENGTH: usize = 10;

/// Unique number per client for tracking the order of packets
pub type SequenceNumber = u64;
pub type TickNumber = u64;
pub type PlayerId = u64;
pub type BulletId = u64;
pub type TimeSecs = f64;

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

    pub fn length_sq(&self) -> f32 {
        self.x * self.x + self.y * self.y
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

#[cfg(feature = "bevy_support")]
impl From<bevy::math::Vec2> for Vec2 {
    fn from(v: bevy::math::Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }
}

#[cfg(feature = "bevy_support")]
impl From<Vec2> for bevy::math::Vec2 {
    fn from(v: Vec2) -> Self {
        bevy::math::Vec2::new(v.x, v.y)
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
    pub velocity: f32, // can only move forward and backward based on the rotation
    pub rotation: f32, // in radians
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
            velocity: 0.0,
            rotation: 0.0,
            health: max_health,
            max_health,
            score: 0,
            life: LifeState::Alive,
            last_processed_input: 0,
        }
    }

    pub fn apply_movement(&mut self, input: &ClientInput, delta_time: f32) {
        // Update rotation
        self.rotation = input.rotation;

        // Apply velocity
        let velocity_mul = match input.move_direction {
            MoveDirection::Forward => 1.0,
            MoveDirection::Backward => -1.0,
            MoveDirection::None => 0.0,
        };

        self.velocity = PLAYER_MOVE_SPEED * velocity_mul;

        // Update position
        let move_dir = Vec2::new(input.rotation.cos(), input.rotation.sin());

        self.position.x += move_dir.x * self.velocity * delta_time;
        self.position.y += move_dir.y * self.velocity * delta_time;

        // Clamp to world bounds
        self.position.x = self
            .position
            .x
            .clamp(WORLD_MIN_X_PADDED, WORLD_MAX_X_PADDED);
        self.position.y = self
            .position
            .y
            .clamp(WORLD_MIN_Y_PADDED, WORLD_MAX_Y_PADDED);

        self.last_processed_input = input.sequence;
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
    pub predicted_time: SystemTime, // Client's predicted time
    pub sequence: SequenceNumber,   // Monotonic input sequence number

    pub move_direction: MoveDirection,
    pub rotation: f32,
    pub shoot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveDirection {
    Forward,
    Backward,
    None,
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
        player: PlayerState,
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
    pub player_name: String,
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

    pub players: Vec<PlayerState>,
    pub bullets: Vec<BulletState>,
    pub scoreboard: Vec<ScoreEntry>,
}

// ===== Server-side Lag Compensator ===== //
pub struct LagCompensator {
    snapshots: VecDeque<LagCompensatorSnapshot>,
}

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
                    LifeState::Dead { .. } => None,
                })
                .collect(),
        };

        // println!("record tick current len: {:?}", self.snapshots.len());

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
    pub fn rewind_to_time(&self, target_time: SystemTime) -> Option<HashMap<PlayerId, Vec2>> {
        if self.snapshots.len() < 2 {
            return Some(self.snapshots.back()?.player_positions.clone());
        }

        let idx = match self
            .snapshots
            .binary_search_by(|s| s.time.cmp(&target_time))
        {
            Ok(i) => return Some(self.snapshots[i].player_positions.clone()),
            Err(i) => i,
        };

        if idx == 0 {
            return Some(self.snapshots[0].player_positions.clone());
        }

        if idx >= self.snapshots.len() {
            return Some(self.snapshots.back()?.player_positions.clone());
        }

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

        Some(result)
    }
}

// ===== Client-side Prediction State ===== //
pub struct PredictionState {
    pending_inputs: VecDeque<ClientInput>,
}

impl PredictionState {
    pub fn new() -> Self {
        Self {
            pending_inputs: VecDeque::new(),
        }
    }

    /// Store a predicted state
    pub fn push_prediction(&mut self, input: ClientInput) {
        self.pending_inputs.push_front(input);
        if self.pending_inputs.len() > 60 {
            self.pending_inputs.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.pending_inputs.clear();
    }

    /// Reconcile with server snapshot
    /// Returns the corrected state after replaying unacknowledged inputs
    pub fn reconcile(
        &mut self,
        local_player: &mut Option<PlayerState>,
        server_state: &PlayerState,
    ) {
        // Remove acknowledged inputs
        self.pending_inputs
            .retain(|input| input.sequence > server_state.last_processed_input);

        // Build corrected state based on the server state
        let mut corrected_state = server_state.clone();
        for input in &self.pending_inputs {
            corrected_state.apply_movement(&input, TICK_DURATION.as_secs_f32());
        }

        // Check for prediction error
        let prediction_error = if let Some(local) = &local_player {
            let dx = local.position.x - corrected_state.position.x;
            let dy = local.position.y - corrected_state.position.y;
            (dx * dx + dy * dy).sqrt()
        } else {
            0.0
        };

        // If error is significant, correct it
        const ERROR_THRESHOLD: f32 = 2.0; // 2 units

        if prediction_error > ERROR_THRESHOLD {
            println!(
                "Prediction error: {:.2} units, reconciling...",
                prediction_error
            );

            *local_player = Some(corrected_state);
        } else {
            // Small error, just update metadata
            if let Some(local) = local_player {
                local.health = corrected_state.health;
                local.score = corrected_state.score;
                local.life = corrected_state.life.clone();
            }
        }
    }
}

// ===== Client-side Entity Interpolation ===== //
pub struct InterpolationBuffer {
    snapshots: VecDeque<(Instant, ViewSnapshot)>,
}

impl InterpolationBuffer {
    pub fn new() -> Self {
        Self {
            snapshots: VecDeque::new(),
        }
    }

    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// Add a new snapshot
    pub fn push_snapshot(&mut self, snapshot: ViewSnapshot) {
        let now = Instant::now();
        self.snapshots.push_back((now, snapshot));

        const MARGIN: Duration = Duration::from_millis(100);

        // Keep only snapshots that's within the delay
        while let Some((time, _)) = self.snapshots.front() {
            if now - *time > INTERPOLATION_DELAY + MARGIN {
                self.snapshots.pop_front();
            } else {
                break;
            }
        }
    }

    /// Get interpolated state for rendering
    /// Render time is current time minus interpolation delay
    pub fn interpolate(&self) -> Option<ViewSnapshot> {
        if self.snapshots.len() < 2 {
            return self.snapshots.back().map(|(_, s)| s.clone());
        }

        let render_time = Instant::now() - INTERPOLATION_DELAY;

        // Find two snapshots to interpolate between
        let mut from_snapshot = None;
        let mut to_snapshot = None;

        for i in 0..self.snapshots.len() - 1 {
            let (time1, snap1) = &self.snapshots[i];
            let (time2, snap2) = &self.snapshots[i + 1];

            if *time1 <= render_time && render_time <= *time2 {
                from_snapshot = Some((time1, snap1));
                to_snapshot = Some((time2, snap2));
                break;
            }
        }

        match (from_snapshot, to_snapshot) {
            (Some((time1, snap1)), Some((time2, snap2))) => {
                let total_duration = time2.duration_since(*time1).as_secs_f32();
                let elapsed = render_time.duration_since(*time1).as_secs_f32();
                let t = (elapsed / total_duration).clamp(0.0, 1.0);

                Some(Self::lerp_snapshots(snap1, snap2, t))
            }
            _ => self.snapshots.back().map(|(_, s)| s.clone()),
        }
    }

    /// Linear interpolation between two snapshots
    fn lerp_snapshots(from: &ViewSnapshot, to: &ViewSnapshot, t: f32) -> ViewSnapshot {
        let mut result = from.clone();

        let to_players = to
            .players
            .iter()
            .map(|p| (p.id, p))
            .collect::<HashMap<_, _>>();

        let to_bullets = to
            .bullets
            .iter()
            .map(|p| (p.id, p))
            .collect::<HashMap<_, _>>();

        // Interpolate player positions
        for player in &mut result.players {
            if let Some(to_player) = to_players.get(&player.id) {
                player.position.lerp_mut(to_player.position, t)
            };
        }

        // Interpolate bullet positions
        for bullet in &mut result.bullets {
            if let Some(to_bullet) = to_bullets.get(&bullet.id) {
                bullet.position.lerp_mut(to_bullet.position, t)
            };
        }

        result
    }
}

pub trait Lerp {
    fn lerp(from: Self, to: Self, t: f32) -> Self;
    fn lerp_mut(&mut self, to: Self, t: f32);
}

impl<T> Lerp for T
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<f32, Output = T>,
{
    fn lerp(from: T, to: T, t: f32) -> T {
        from * (1.0 - t) + to * t // Same as `from + (to - from) * t`
    }

    fn lerp_mut(&mut self, to: Self, t: f32) {
        *self = Self::lerp(*self, to, t)
    }
}
