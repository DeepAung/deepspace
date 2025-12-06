use std::{
    array,
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    net::SocketAddr,
    time::Duration,
};

use shared::{
    BulletId, BulletState, ClientConnection, ClientInput, DeltaSnapshot, LifeState, PlayerId,
    PlayerState, SCOREBOARD_LENGTH, ScoreEntry, ServerPacket, Snapshot, TickNumber, Vec2,
};

// ===== Default Constants ===== //
pub const TICK_RATE: u32 = 60; // Server simulation rate (Hz)
pub const TICK_DURATION: Duration = Duration::from_nanos(1_000_000_000 / TICK_RATE as u64);

pub const SNAPSHOT_RATE: u32 = 20; // Snapshots per second to each client
pub const SNAPSHOT_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / SNAPSHOT_RATE as u64);

// ===== World Constants ===== //
const WORLD_WIDTH: f32 = 2000.0;
const WORLD_HEIGHT: f32 = 2000.0;
const WORLD_PADDING: f32 = 20.0;

const WORLD_MIN_X: f32 = -WORLD_WIDTH / 2.0;
const WORLD_MAX_X: f32 = WORLD_WIDTH / 2.0;
const WORLD_MIN_Y: f32 = -WORLD_HEIGHT / 2.0;
const WORLD_MAX_Y: f32 = WORLD_HEIGHT / 2.0;

const WORLD_MIN_X_PADDED: f32 = WORLD_MIN_X + WORLD_PADDING;
const WORLD_MAX_X_PADDED: f32 = WORLD_MAX_X - WORLD_PADDING;
const WORLD_MIN_Y_PADDED: f32 = WORLD_MIN_Y + WORLD_PADDING;
const WORLD_MAX_Y_PADDED: f32 = WORLD_MAX_Y - WORLD_PADDING;

// ===== Player Constants ===== //
const PLAYER_MOVE_SPEED: f32 = 300.0;
const PLAYER_MAX_HEALTH: i32 = 100;
const PLAYER_RESPAWN_TIME_SECS: f32 = 5.0;
const MAX_PLAYERS: usize = 20_000;
const PLAYER_RADIUS: f32 = 20.0;

// ===== Bullet Constants ===== //
const BULLET_LIFETIME: Duration = Duration::from_secs(3); // 3 seconds
const BULLET_LIFETIME_TICKS: TickNumber = BULLET_LIFETIME.as_secs() * (TICK_RATE as u64);
const BULLET_DAMAGE: i32 = 20;
const MAX_BULLETS: usize = 100_000;
const BULLET_RADIUS: f32 = 5.0;

// ===== Game Server ===== //
#[derive(Debug, Clone)]
pub struct GameServer {
    players: HashMap<PlayerId, PlayerState>,
    bullets: Vec<BulletState>,

    next_player_id: u64,
    next_bullet_id: u64,

    clients: HashMap<SocketAddr, ClientConnection>,
    addr_to_player_id: HashMap<SocketAddr, PlayerId>,

    current_tick: TickNumber,

    input_queue: Vec<(SocketAddr, ClientInput)>,
}

impl GameServer {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            bullets: Vec::new(),

            next_player_id: 0,
            next_bullet_id: 0,

            clients: HashMap::new(),
            addr_to_player_id: HashMap::new(),

            current_tick: 0,

            input_queue: Vec::new(),
        }
    }

    pub fn get_clients(&self) -> Vec<SocketAddr> {
        self.clients.keys().cloned().collect::<Vec<_>>()
    }

    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    pub fn handle_connect(&mut self, addr: SocketAddr, player_name: String) -> ServerPacket {
        if self.players.len() >= MAX_PLAYERS {
            return ServerPacket::ConnectionRejected {
                reason: "Server full".to_string(),
            };
        }

        if let None = self.clients.get(&addr) {
            return ServerPacket::ConnectionRejected {
                reason: "Client already connected".to_string(),
            };
        }

        let player_id = self.generate_next_player_id();
        let spawn_pos = Self::generate_spawn_pos();
        let player = PlayerState::new(player_id, player_name, spawn_pos, PLAYER_MAX_HEALTH);

        self.clients.insert(addr, ClientConnection::new(player_id));
        self.addr_to_player_id.insert(addr, player_id);
        self.players.insert(player_id, player);

        let full_snapshot = self.generate_full_snapshot();

        ServerPacket::ConnectionAccepted {
            player_id,
            full_snapshot,
        }
    }

    // TODO: avoid spawn near enemy
    fn generate_spawn_pos() -> Vec2 {
        let x = rand::random_range((WORLD_MIN_X_PADDED)..=(WORLD_MAX_X_PADDED));
        let y = rand::random_range((WORLD_MIN_Y_PADDED)..=(WORLD_MAX_Y_PADDED));
        Vec2::new(x, y)
    }

    fn generate_next_player_id(&mut self) -> PlayerId {
        // INFO: A unique ID is guaranteed to be found because the player count is limited by MAX_PLAYERS.
        while self.players.contains_key(&self.next_player_id) {
            self.next_player_id += 1;
        }

        self.next_player_id
    }

    fn generate_next_bullet_id(&mut self) -> anyhow::Result<BulletId> {
        todo!()
    }

    pub fn handle_disconnect(&mut self, addr: SocketAddr) {
        if let Some(player_id) = self.addr_to_player_id.remove(&addr) {
            self.clients.remove(&addr);
            self.players.remove(&player_id);
        }
    }

    pub fn queue_input(&mut self, addr: SocketAddr, input: ClientInput) {
        self.input_queue.push((addr, input))
    }

    pub fn tick(&mut self, delta_time: f32) -> Vec<SocketAddr> {
        self.process_inputs(delta_time);

        self.update_bullets(delta_time);

        self.check_collisions();

        self.update_respawn(delta_time);

        let disconnected_clients = self.cleanup_disconnected_clients();

        self.input_queue.clear();

        disconnected_clients
    }

    fn process_inputs(&mut self, delta_time: f32) {
        self.input_queue.sort_by_key(|a| a.1.sequence);

        for (addr, input) in &self.input_queue {
            let Some(player_id) = self.addr_to_player_id.get(addr) else {
                continue;
            };

            let Some(player) = self.players.get_mut(player_id) else {
                continue;
            };

            match player.life {
                LifeState::Alive => {}
                LifeState::Dead { respawn_time: _ } => continue,
            };

            Self::apply_movement(player, input, delta_time);
            if input.shoot {
                Self::handle_shoot(player, input);
            }

            player.last_processed_input = input.sequence;
        }
    }

    fn apply_movement(player: &mut PlayerState, input: &ClientInput, delta_time: f32) {
        // Normalize movement direction
        let move_dir = input.move_direction.clone().normalized();

        // Apply velocity
        player.velocity = Vec2::new(
            move_dir.x * PLAYER_MOVE_SPEED,
            move_dir.y * PLAYER_MOVE_SPEED,
        );

        // Update position
        player.position.x += player.velocity.x * delta_time;
        player.position.y += player.velocity.y * delta_time;

        // Clamp to world bounds
        player.position.x = player
            .position
            .x
            .clamp(WORLD_MIN_X_PADDED, WORLD_MAX_X_PADDED);
        player.position.y = player
            .position
            .y
            .clamp(WORLD_MIN_Y_PADDED, WORLD_MAX_Y_PADDED);
    }

    fn handle_shoot(player: &mut PlayerState, input: &ClientInput) {
        // Spawn bullet object, with lag compensation
        todo!()
    }

    fn update_bullets(&mut self, delta_time: f32) {
        for bullet in &mut self.bullets {
            bullet.position.x += bullet.velocity.x * delta_time;
            bullet.position.y += bullet.velocity.y * delta_time;
        }

        // Remove old or out-of-bounds bullets
        self.bullets.retain(|bullet| {
            let age = self.current_tick - bullet.spawn_tick;
            let in_bounds = WORLD_MIN_X <= bullet.position.x
                && bullet.position.x <= WORLD_MAX_X
                && WORLD_MIN_Y <= bullet.position.y
                && bullet.position.y <= WORLD_MAX_Y;

            age < BULLET_LIFETIME_TICKS && in_bounds
        });

        // Limit total bullets
        if self.bullets.len() > MAX_BULLETS {
            self.bullets.drain(0..(self.bullets.len() - MAX_BULLETS));
        }
    }

    fn check_collisions(&mut self) {
        let mut hits = Vec::new();

        // TODO: optimize this nested loop by using quad tree
        for bullet in &self.bullets {
            for player in self.players.values() {
                match player.life {
                    LifeState::Alive => {}
                    LifeState::Dead { respawn_time: _ } => continue,
                };

                if bullet.owner_id == player.id {
                    continue;
                }

                let dx = player.position.x - bullet.position.x;
                let dy = player.position.y - bullet.position.y;
                let dist_sq = dx * dx + dy * dy;
                let hit_dist = PLAYER_RADIUS + BULLET_RADIUS;

                if dist_sq < hit_dist * hit_dist {
                    hits.push((bullet.id, player.id, bullet.owner_id, bullet.damage));
                }
            }
        }

        for (bullet_id, target_id, shooter_id, damage) in hits {
            self.bullets.retain(|b| b.id != bullet_id); // TODO: optimize this

            if let Some(target) = self.players.get_mut(&target_id) {
                target.health -= damage;
                if target.health <= 0 {
                    self.handle_player_death(target_id, shooter_id);
                }
            };
        }
    }

    fn handle_player_death(&mut self, victim_id: PlayerId, killer_id: PlayerId) {
        if let Some(victim) = self.players.get_mut(&victim_id) {
            victim.life = LifeState::Dead {
                respawn_time: PLAYER_RESPAWN_TIME_SECS,
            };
            victim.health = 0;
        }

        if let Some(killer) = self.players.get_mut(&killer_id) {
            killer.score += 1;
        }
    }

    fn update_respawn(&mut self, delta_time: f32) {
        for player in self.players.values_mut() {
            match &mut player.life {
                LifeState::Alive => continue,
                LifeState::Dead { respawn_time } => {
                    let new_time = *respawn_time - delta_time;
                    if new_time <= 0.0 {
                        player.position = Self::generate_spawn_pos();
                        player.velocity = Vec2::new(0.0, 0.0);
                        player.health = player.max_health;
                        player.score = 0;
                        player.life = LifeState::Alive;
                    } else {
                        *respawn_time = new_time;
                    }
                }
            };
        }
    }

    fn cleanup_disconnected_clients(&mut self) -> Vec<SocketAddr> {
        let mut to_remove = Vec::new();

        for (addr, client) in &self.clients {
            if client.is_timed_out() {
                to_remove.push(*addr);
            }
        }

        for addr in &to_remove {
            self.handle_disconnect(*addr);
        }

        to_remove
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
        // 1. Initialize a Min-Heap of size K (SCOREBOARD_LENGTH)
        // We use Reverse to make the Max-Heap (BinaryHeap) act like a Min-Heap (it pops the smallest item)
        let mut min_heap: BinaryHeap<Reverse<ScoreEntry>> =
            BinaryHeap::with_capacity(SCOREBOARD_LENGTH + 1);

        for player in self.players.values() {
            let entry = ScoreEntry {
                player_id: player.id,
                score: player.score,
            };

            // 2. Push the current entry into the heap
            min_heap.push(Reverse(entry));

            // 3. Maintain size K: If the heap is too large, pop the smallest element (the reverse is true for a Max-Heap)
            if min_heap.len() > SCOREBOARD_LENGTH {
                min_heap.pop();
            }
        }

        // 4. Extract and sort the results
        // After the loop, the heap contains the K highest scores, but they are unordered.
        // We need to move them into a Vec and sort them properly (highest score first)
        let mut final_entries: Vec<ScoreEntry> = min_heap
            .into_iter()
            .map(|reverse_entry| reverse_entry.0) // Unwrap the Reverse wrapper
            .collect();

        final_entries.sort_by_key(|a| Reverse(a.score));

        // 5. Convert to the fixed-size array.
        array::from_fn(|i| final_entries[i].clone())
    }
}
