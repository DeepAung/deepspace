use anyhow::bail;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time as tokiotime;

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

    pub async fn connect(&mut self, socket: &UdpSocket, player_name: String) -> anyhow::Result<()> {
        if self.connected {
            bail!("Client already connected")
        }

        let packet = ClientPacket::Connect { player_name };
        let packet_encoded = bincode::serde::encode_to_vec(packet, self.bincode_cfg)?;
        socket.send_to(&packet_encoded, self.server_addr).await?;

        Ok(())
    }

    pub async fn disconnect(&mut self, socket: &UdpSocket) -> anyhow::Result<()> {
        if !self.connected {
            bail!("Client not connected yet")
        }

        let packet = ClientPacket::Disconnect;
        let packet_encoded = bincode::serde::encode_to_vec(packet, self.bincode_cfg)?;
        socket.send_to(&packet_encoded, self.server_addr).await?;

        Ok(())
    }

    pub async fn send_input(
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
        socket.send_to(&packet_encoded, self.server_addr).await?;

        Ok(())
    }

    async fn time_sync(&self, socket: &UdpSocket) -> anyhow::Result<()> {
        if !self.connected {
            bail!("Client not connected yet")
        }

        let client_send_time = SystemTime::now();
        let packet = ClientPacket::TimeSync { client_send_time };
        let packet_encoded = bincode::serde::encode_to_vec(packet, self.bincode_cfg)?;

        socket.send_to(&packet_encoded, self.server_addr).await?;

        Ok(())
    }

    pub async fn run_loop(self, socket: Arc<UdpSocket>) {
        println!("Start client run loop");

        // TODO: should I use RwLock instead?
        let game_client = Arc::new(Mutex::new(self));

        // Time Sync Loop
        let time_sync_socket = Arc::clone(&socket);
        let time_sync_game_client = Arc::clone(&game_client);
        tokio::spawn(async move {
            let mut ticker = tokiotime::interval(TIME_SYNC_DURATION);

            loop {
                ticker.tick().await;

                let game = time_sync_game_client.lock().await;

                game.time_sync(&time_sync_socket).await.unwrap();
            }
        });

        // Receiver loop
        let mut buf = vec![0u8; MAX_PACKET_SIZE];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, _)) => {
                    let client_recv_time = SystemTime::now();
                    let mut game = game_client.lock().await;

                    let packet: ServerPacket =
                        match bincode::serde::decode_from_slice(&buf[..len], game.bincode_cfg) {
                            Ok((p, _)) => p,
                            Err(e) => {
                                eprintln!("Error decoding server packet: {}", e);
                                continue;
                            }
                        };

                    match packet {
                        ServerPacket::ConnectionAccepted { player_id } => {
                            game.handle_connection_accepted(player_id)
                        }
                        ServerPacket::ConnectionRejected { reason } => {
                            game.handle_connection_rejected(reason)
                        }
                        ServerPacket::Disconnect { reason } => game.handle_disconnect(reason),
                        ServerPacket::ViewSnapshot(view_snapshot) => {
                            game.handle_snapshot(view_snapshot)
                        }
                        ServerPacket::TimeSync {
                            client_send_time,
                            server_recv_time,
                            server_send_time,
                        } => game.handle_time_sync(
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

    fn handle_connection_accepted(&mut self, player_id: PlayerId) {
        self.player_id = Some(player_id);
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
}

// #[derive(Debug, Clone)]
// pub struct RenderState {
//     pub local_player: Option<PlayerState>,
//     pub other_players: Vec<PlayerState>,
//     pub bullets: Vec<BulletState>,
//     pub scoreboard: Vec<ScoreEntry>,
// }
//
// pub async fn run_client_receiver(
//     mut client: GameClient,
//     socket: Arc<UdpSocket>,
// ) -> anyhow::Result<()> {
//     todo!()
// }
