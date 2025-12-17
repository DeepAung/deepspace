use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, hash_map::Entry},
    net::SocketAddr,
    time::{Instant, SystemTime},
};

use shared::*;

// ===== Game Server ===== //
pub struct GameServer {
    players: HashMap<PlayerId, PlayerState>,
    bullets: Vec<BulletState>,

    clients: HashMap<SocketAddr, ClientConnection>,
    addr_to_player_id: HashMap<SocketAddr, PlayerId>,

    current_tick: TickNumber,
    next_player_id: u64,
    next_bullet_id: u64,

    lag_compensator: LagCompensator,

    input_queue: Vec<(SocketAddr, ClientInput)>,
}

impl GameServer {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            bullets: Vec::new(),

            clients: HashMap::new(),
            addr_to_player_id: HashMap::new(),

            current_tick: 0,
            next_player_id: 0,
            next_bullet_id: 0,

            lag_compensator: LagCompensator::new(),

            input_queue: Vec::new(),
        }
    }

    pub fn handle_connect(&mut self, addr: SocketAddr, player_name: String) -> ServerPacket {
        if self.players.len() >= MAX_PLAYERS {
            return ServerPacket::ConnectionRejected {
                reason: "Server full".to_string(),
            };
        }

        if let Entry::Occupied(_) = self.clients.entry(addr) {
            return ServerPacket::ConnectionRejected {
                reason: "Client already connected".to_string(),
            };
        }

        let player_id = self.generate_next_player_id();
        let spawn_pos = Self::generate_spawn_pos();
        let player = PlayerState::new(player_id, player_name, spawn_pos, PLAYER_MAX_HEALTH);

        self.clients.insert(addr, ClientConnection::new(player_id));
        self.addr_to_player_id.insert(addr, player_id);
        self.players.insert(player_id, player.clone());

        ServerPacket::ConnectionAccepted { player }
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

    pub fn handle_disconnect(&mut self, addr: SocketAddr) {
        if let Some(player_id) = self.addr_to_player_id.remove(&addr) {
            self.clients.remove(&addr);
            self.players.remove(&player_id);
        }
    }

    pub fn queue_input(&mut self, addr: SocketAddr, input: ClientInput) {
        self.update_last_heard(addr);

        self.input_queue.push((addr, input))
    }

    pub fn handle_time_sync(
        &mut self,
        addr: SocketAddr,
        client_send_time: SystemTime,
        server_recv_time: SystemTime,
    ) -> ServerPacket {
        self.update_last_heard(addr);

        let server_send_time = SystemTime::now();

        ServerPacket::TimeSync {
            client_send_time,
            server_recv_time,
            server_send_time,
        }
    }

    fn update_last_heard(&mut self, addr: SocketAddr) {
        if let Some(client) = self.clients.get_mut(&addr) {
            client.last_heard = Instant::now();
        }
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

        for (addr, input) in self.input_queue.clone() {
            let Some(player_id) = self.addr_to_player_id.get(&addr) else {
                continue;
            };

            let Some(player) = self.players.get_mut(player_id) else {
                continue;
            };

            match player.life {
                LifeState::Alive => {}
                LifeState::Dead { respawn_time: _ } => continue,
            };

            player.apply_movement(&input, delta_time);
            if input.shoot {
                self.handle_shoot(*player_id, &input);
            }
        }
    }

    fn handle_shoot(&mut self, shooter_id: PlayerId, input: &ClientInput) {
        let Some(shooter) = self.players.get(&shooter_id) else {
            return;
        };

        let rewound_positions = self.lag_compensator.rewind_to_time(input.prediected_time);

        // TODO: check if bullet_id already exist
        let bullet_id = self.next_bullet_id;
        self.next_bullet_id += 1;

        let bullet = BulletState {
            id: bullet_id,
            owner_id: shooter_id,
            position: shooter.position,
            velocity: shooter.velocity.normalized() * BULLET_SPEED,
            spawn_tick: self.current_tick,
            damage: BULLET_DAMAGE,
        };

        if let Some(target_id) = self.check_instant_hit(shooter_id, &bullet, &rewound_positions) {
            // Hit detected, Apply damage immediately
            if let Some(target) = self.players.get_mut(&target_id) {
                target.health -= bullet.damage;

                if target.health <= 0 {
                    self.handle_player_death(target_id, shooter_id);
                }
            }
        } else {
            // No instant hit, spawn the bullet projectile
            self.bullets.push(bullet);
        }
    }

    fn check_instant_hit(
        &self,
        shooter_id: PlayerId,
        bullet: &BulletState,
        rewound_positions: &HashMap<PlayerId, Vec2>,
    ) -> Option<PlayerId> {
        let shoot_pos = bullet.position;
        let shoot_dir = bullet.velocity.normalized();

        let mut closest_hit = None;
        let mut closest_dist = BULLET_INSTANT_HIT_MAX_RANGE;

        for (&player_id, &pos) in rewound_positions {
            if player_id == shooter_id {
                continue; // Don't shoot yourself
            }

            let Some(dist) = Self::ray_intersects_circle(
                shoot_pos,
                shoot_dir,
                pos,
                PLAYER_RADIUS,
                BULLET_INSTANT_HIT_MAX_RANGE,
            ) else {
                continue;
            };

            if dist < closest_dist {
                closest_hit = Some(player_id);
                closest_dist = dist;
            }
        }

        closest_hit
    }

    fn ray_intersects_circle(
        ray_origin: Vec2,
        ray_direction: Vec2, // Must be normalized
        circle_origin: Vec2,
        circle_radius: f32,
        max_range: f32,
    ) -> Option<f32> {
        let radius_sq = circle_radius * circle_radius;

        let to_target = circle_origin - ray_origin;

        let t = to_target.dot(&ray_direction);

        if t < 0.0 || t > max_range {
            return None;
        }

        let to_target_len = to_target.length();
        let to_target_len_sq = to_target_len * to_target_len;
        let dist_sq_to_ray = to_target_len_sq - (t * t);

        if dist_sq_to_ray >= 0.0 && dist_sq_to_ray <= radius_sq {
            return Some(t);
        }

        None
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

    pub fn generate_all_players_snapshot(&self) -> Vec<(SocketAddr, ViewSnapshot)> {
        let mut result = Vec::new();

        let scoreboard = self.generate_scoreboard();

        for (addr, viewer_id) in &self.addr_to_player_id {
            let Some(viewer) = self.players.get(viewer_id) else {
                continue;
            };

            let inbound_players = self
                .players
                .values()
                .filter(|p| p.position.inbound(&viewer.position, &VIEW_SIZE))
                .cloned()
                .collect::<Vec<_>>();

            let inbound_bullets = self
                .bullets
                .iter()
                .filter(|b| b.position.inbound(&viewer.position, &VIEW_SIZE))
                .cloned()
                .collect::<Vec<_>>();

            let snapshot = ViewSnapshot {
                viewer_position: viewer.position,
                tick: self.current_tick,
                players: inbound_players,
                bullets: inbound_bullets,
                scoreboard: scoreboard.clone(),
            };

            result.push((*addr, snapshot));
        }

        result
    }

    fn generate_scoreboard(&self) -> Vec<ScoreEntry> {
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

        final_entries
    }
}
