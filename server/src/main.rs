use shared::{ClientPacket, ServerPacket};
use std::{io, sync::Arc};
use tokio::{net::UdpSocket, sync::Mutex, time};

use crate::game_server::{GameServer, SNAPSHOT_INTERVAL, TICK_DURATION};

mod game_server;

const PORT: u16 = 8080;

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = Arc::new(UdpSocket::bind(("0.0.0.0", PORT)).await?);
    println!("Server listening on port {}", PORT);

    let game_state = Arc::new(Mutex::new(GameServer::new()));
    let bincode_cfg = bincode::config::standard();

    // Game tick loop
    let game_socket = Arc::clone(&socket);
    let game_tick_state = Arc::clone(&game_state);
    tokio::spawn(async move {
        let mut ticker = time::interval(TICK_DURATION);

        loop {
            ticker.tick().await;

            let mut game = game_tick_state.lock().await;
            let disconnected_clients = game.tick(TICK_DURATION.as_secs_f32());

            if disconnected_clients.is_empty() {
                continue;
            }

            let packet = ServerPacket::Disconnect {
                reason: "Client timed out".to_string(),
            };
            let packet_encoded = bincode::serde::encode_to_vec(packet, bincode_cfg).unwrap();

            for addr in disconnected_clients {
                game_socket.send_to(&packet_encoded, addr).await.unwrap();
            }
        }
    });

    // Snapshot loop
    let write_socket = Arc::clone(&socket);
    let game_snapshot_state = Arc::clone(&game_state);
    tokio::spawn(async move {
        let mut ticker = time::interval(SNAPSHOT_INTERVAL);
        let mut buf = Vec::new(); // TODO: maybe use something else instead
        let mut base_tick: u64 = 0;

        loop {
            ticker.tick().await;

            let game = game_snapshot_state.lock().await;

            let delta_snapshot = game.generate_delta_snapshot(base_tick);
            base_tick = game.current_tick();

            let clients = game.get_clients();

            drop(game);

            bincode::serde::encode_into_slice(
                ServerPacket::DeltaSnapshot(delta_snapshot),
                &mut buf,
                bincode_cfg,
            )
            .unwrap();

            // Send delta snapshot to every client
            for addr in clients {
                write_socket.send_to(&buf, addr).await.unwrap();
            }
        }
    });

    let mut buf = Vec::new(); // TODO: maybe use something else instead

    // Receiving loop
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, addr)) => {
                let packet: ClientPacket =
                    match bincode::serde::decode_from_slice(&buf[..len], bincode_cfg) {
                        Ok((p, _)) => p,
                        Err(e) => {
                            eprintln!("Error decoding client packet: {}", e);
                            continue;
                        }
                    };

                let mut game = game_state.lock().await;

                match packet {
                    ClientPacket::Connect { player_name } => {
                        let packet = game.handle_connect(addr, player_name);

                        let packet_encoded =
                            bincode::serde::encode_to_vec(packet, bincode_cfg).unwrap();

                        socket.send_to(&packet_encoded, addr).await.unwrap();
                    }
                    ClientPacket::Disconnect => {
                        game.handle_disconnect(addr);
                        let packet = ServerPacket::Disconnect {
                            reason: "Client request disconnect".to_string(),
                        };

                        let packet_encoded =
                            bincode::serde::encode_to_vec(packet, bincode_cfg).unwrap();

                        socket.send_to(&packet_encoded, addr).await.unwrap();
                    }
                    ClientPacket::Input(client_input) => {
                        game.queue_input(addr, client_input);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error receiving packet: {}", e);
            }
        }
    }
}
