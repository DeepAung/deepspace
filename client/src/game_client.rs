use anyhow::bail;
use crossbeam::channel::Sender;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use shared::*;

pub struct GameClient {
    // Connection
    server_addr: SocketAddr,
    player_id: Option<PlayerId>,
    connected: bool,

    // Packet encode/decode config
    bincode_cfg: bincode::config::Configuration,

    // Local state
    local_player: Option<PlayerState>,
    other_players: Vec<PlayerState>,
    bullets: Vec<BulletState>,
    scoreboard: Vec<ScoreEntry>,

    // Client-side prediction
    prediction: PredictionState,
    next_input_sequence: SequenceNumber,

    // Entity interpolation
    interpolation_buffer: InterpolationBuffer,

    // Timing
    round_trip_time: TimeSecs,
    clock_offset: TimeSecs,
}

impl GameClient {
    pub fn new(server_addr: SocketAddr) -> Self {
        Self {
            server_addr,
            player_id: None,
            connected: false,
            bincode_cfg: bincode::config::standard(),
            local_player: None,
            other_players: Vec::new(),
            bullets: Vec::new(),
            scoreboard: Vec::new(),
            prediction: PredictionState::new(),
            next_input_sequence: 0,
            interpolation_buffer: InterpolationBuffer::new(),
            round_trip_time: 0.0,
            clock_offset: 0.0,
        }
    }

    pub fn connected(&self) -> bool {
        self.connected
    }

    pub fn connect(&mut self, socket: &UdpSocket, player_name: String) -> anyhow::Result<()> {
        if self.connected {
            bail!("Client already connected")
        }

        let packet = ClientPacket::Connect { player_name };
        let packet_encoded = bincode::serde::encode_to_vec(packet, self.bincode_cfg)?;
        socket.send_to(&packet_encoded, self.server_addr)?;

        Ok(())
    }

    pub fn disconnect(&mut self, socket: &UdpSocket) -> anyhow::Result<()> {
        if !self.connected {
            bail!("Client not connected yet")
        }

        let packet = ClientPacket::Disconnect;
        let packet_encoded = bincode::serde::encode_to_vec(packet, self.bincode_cfg)?;
        socket.send_to(&packet_encoded, self.server_addr)?;

        Ok(())
    }

    pub fn send_input(
        &mut self,
        socket: &UdpSocket,
        move_direction: Vec2,
        shoot: bool,
    ) -> anyhow::Result<()> {
        if !self.connected {
            bail!("Client not connected yet")
        }

        // Create input
        let prediected_time = SystemTime::now() + Duration::from_secs_f64(self.clock_offset);
        let input = ClientInput {
            prediected_time,
            sequence: self.next_input_sequence,
            move_direction,
            shoot,
        };
        self.next_input_sequence += 1;

        // Store for reconciliation
        self.prediction.push_prediction(input.clone());

        // Apply input immediately to local player, reconcile later in handle_snapshot
        if let Some(local_player) = &mut self.local_player {
            local_player.apply_movement(&input, TICK_DURATION.as_secs_f32());
        }

        // Send to server
        let packet = ClientPacket::Input(input);
        let packet_encoded = bincode::serde::encode_to_vec(packet, self.bincode_cfg)?;
        socket.send_to(&packet_encoded, self.server_addr)?;

        Ok(())
    }

    pub fn time_sync(&self, socket: &UdpSocket) -> anyhow::Result<()> {
        if !self.connected {
            bail!("Client not connected yet")
        }

        let client_send_time = SystemTime::now();
        let packet = ClientPacket::TimeSync { client_send_time };
        let packet_encoded = bincode::serde::encode_to_vec(packet, self.bincode_cfg)?;

        socket.send_to(&packet_encoded, self.server_addr)?;

        Ok(())
    }

    pub fn recv_loop(
        game_client: Arc<Mutex<GameClient>>,
        socket: Arc<UdpSocket>,
        tx: Sender<RenderState>,
    ) {
        let mut buf = vec![0u8; MAX_PACKET_SIZE];

        loop {
            println!("TRY RECV FROM BUF");
            match socket.recv_from(&mut buf) {
                Ok((len, _)) => {
                    let client_recv_time = SystemTime::now();

                    let packet: ServerPacket = match bincode::serde::decode_from_slice(
                        &buf[..len],
                        bincode::config::standard(),
                    ) {
                        Ok((p, _)) => p,
                        Err(e) => {
                            eprintln!("Error decoding server packet: {}", e);
                            return;
                        }
                    };

                    println!("Got server packet: {:?}", packet);

                    let mut game_client = game_client.lock().unwrap();

                    match packet {
                        ServerPacket::ConnectionAccepted { player } => {
                            game_client.handle_connection_accepted(player)
                        }
                        ServerPacket::ConnectionRejected { reason } => {
                            game_client.handle_connection_rejected(reason)
                        }
                        ServerPacket::Disconnect { reason } => {
                            game_client.handle_disconnect(reason)
                        }
                        ServerPacket::ViewSnapshot(view_snapshot) => {
                            game_client.handle_snapshot(view_snapshot);
                            let render_state = game_client.get_render_state();
                            drop(game_client);

                            tx.send(render_state).unwrap();
                        }
                        ServerPacket::TimeSync {
                            client_send_time,
                            server_recv_time,
                            server_send_time,
                        } => game_client.handle_time_sync(
                            client_send_time,
                            server_recv_time,
                            server_send_time,
                            client_recv_time,
                        ),
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving packet: {}", e);
                }
            }
        }
    }

    fn handle_connection_accepted(&mut self, player: PlayerState) {
        let player_id = player.id;

        self.player_id = Some(player_id);
        self.local_player = Some(player);
        self.connected = true;

        println!("Connected! Player ID: {}", player_id);
    }

    fn handle_connection_rejected(&mut self, reason: String) {
        eprintln!("Connection rejected: {}", reason)
    }

    fn handle_disconnect(&mut self, reason: String) {
        self.player_id = None;
        self.connected = false;

        self.local_player = None;
        self.other_players.clear();
        self.bullets.clear();
        self.scoreboard.clear();

        self.prediction.clear();
        self.next_input_sequence = 0;

        self.interpolation_buffer.clear();

        println!("Disconnect Player with reason: {}", reason);
    }

    fn handle_snapshot(&mut self, snapshot: ViewSnapshot) {
        // Add to interpolation buffer for other entities
        // TODO: how can i guarantee snapshot order
        self.interpolation_buffer.push_snapshot(snapshot.clone());

        // Find our player in the snapshot
        if let Some(player_id) = self.player_id {
            if let Some(server_player) = snapshot.players.iter().find(|p| p.id == player_id) {
                // RECONCILIATION: Compare with our prediction
                self.prediction
                    .reconcile(&mut self.local_player, server_player);
            }
        }

        // Update other players (will be interpolated for rendering)
        self.other_players = snapshot
            .players
            .iter()
            .filter(|p| Some(p.id) != self.player_id)
            .cloned()
            .collect();

        // Update bullets (immediate, no prediction needed)
        self.bullets = snapshot.bullets;

        // Update scoreboard
        self.scoreboard = snapshot.scoreboard;
    }

    fn handle_time_sync(
        &mut self,
        client_send_time: SystemTime,
        server_recv_time: SystemTime,
        server_send_time: SystemTime,
        client_recv_time: SystemTime,
    ) {
        // TODO: update TimeSync and others to use TimeSecs instead of SystemTime
        let t1 = client_send_time
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let t2 = server_recv_time
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let t3 = server_send_time
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        let t4 = client_recv_time
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();

        self.round_trip_time = (t4 - t1) - (t3 - t2);
        self.clock_offset = (t2 - t1) + (self.round_trip_time / 2.0);
    }

    pub fn get_render_state(&self) -> RenderState {
        // Local player: use predicted state (no interpolation)
        let local_player = self.local_player.clone();

        // Other players: use interpolated state (delayed by 100ms)
        let interpolated_snapshot = self.interpolation_buffer.interpolate();

        let other_players = match interpolated_snapshot {
            Some(snapshot) => snapshot
                .players
                .iter()
                .filter(|p| Some(p.id) != self.player_id)
                .cloned()
                .collect(),
            None => self.other_players.clone(),
        };

        // Bullets: no interpolation (fast-moving, short-lived)
        let bullets = self.bullets.clone();

        RenderState {
            local_player,
            other_players,
            bullets,
            scoreboard: self.scoreboard.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderState {
    pub local_player: Option<PlayerState>,
    pub other_players: Vec<PlayerState>,
    pub bullets: Vec<BulletState>,
    pub scoreboard: Vec<ScoreEntry>,
}
