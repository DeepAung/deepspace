use std::{collections::HashMap, net::SocketAddr, time::Duration};

use shared::{
    BulletState, ClientInput, DeltaSnapshot, PlayerId, PlayerState, SCOREBOARD_LENGTH, ScoreEntry,
    Snapshot, TickNumber,
};

pub const TICK_RATE: u32 = 60; // Server simulation rate (Hz)
pub const TICK_DURATION: Duration = Duration::from_micros(16_667);

pub const SNAPSHOT_RATE: u32 = 20; // Snapshots per second to each client
pub const SNAPSHOT_INTERVAL: Duration = Duration::from_millis(50);

const PLAYER_MAX_HEALTH: i32 = 100;
const PLAYER_RESPAWN_TIME_SECS: f32 = 3.0;
const BULLET_DAMAGE: i32 = 20;

#[derive(Debug, Clone)]
pub struct GameServer {
    players: HashMap<PlayerId, PlayerState>,
    bullets: Vec<BulletState>,

    clients: Vec<SocketAddr>,
    addr_to_player_id: HashMap<SocketAddr, PlayerId>,

    current_tick: TickNumber,

    input_queue: Vec<(SocketAddr, ClientInput)>,
}

impl GameServer {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            bullets: Vec::new(),

            clients: Vec::new(),
            addr_to_player_id: HashMap::new(),

            current_tick: 0,

            input_queue: Vec::new(),
        }
    }

    pub fn get_clients(&self) -> Vec<SocketAddr> {
        self.clients.clone()
    }

    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    pub fn handle_connect(&mut self, addr: SocketAddr, player_name: String) {
        todo!()
    }

    pub fn handle_disconnect(&mut self, addr: SocketAddr) {
        todo!()
    }

    pub fn queue_input(&mut self, addr: SocketAddr, input: ClientInput) {
        todo!()
    }

    pub fn tick(&mut self, delta: f32) {
        todo!()
    }

    pub fn generate_full_snapshot(&self) -> Snapshot {
        Snapshot {
            tick: self.current_tick,
            players: self.players.values().cloned().collect(),
            bullets: self.bullets.clone(),
            scoreboard: self.generate_scoreboard(),
        }
    }

    pub fn generate_delta_snapshot(&self, base_tick: TickNumber) -> DeltaSnapshot {
        todo!()
    }

    fn generate_scoreboard(&self) -> [ScoreEntry; SCOREBOARD_LENGTH] {
        let mut entries: Vec<_> = self
            .players
            .values()
            .map(|p| ScoreEntry {
                player_id: p.id,
                score: p.score,
            })
            .collect();

        entries.sort_by(|a, b| b.score.cmp(&a.score));
        entries.truncate(SCOREBOARD_LENGTH);

        std::array::from_fn(|i| entries.get(i).unwrap().clone())
    }
}
