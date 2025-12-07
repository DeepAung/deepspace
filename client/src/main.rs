use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use shared::{ClientInput, ClientPacket, MAX_PACKET_SIZE, SequenceNumber, ServerPacket, Vec2};
use tokio::net::UdpSocket;

const SERVER_ADDR: &str = "0.0.0.0:8080";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    println!("UDP client bound to: {}", socket.local_addr()?);

    let bincode_cfg = bincode::config::standard();

    // Receiving loop
    let recv_socket = Arc::clone(&socket);
    tokio::spawn(async move {
        let mut buf = [0u8; MAX_PACKET_SIZE];

        loop {
            match recv_socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let packet: ServerPacket =
                        match bincode::serde::decode_from_slice(&buf[..len], bincode_cfg) {
                            Ok((p, _)) => p,
                            Err(e) => {
                                eprintln!("Error decoding server packet: {}", e);
                                continue;
                            }
                        };

                    println!("From addr {}", addr);
                    println!("Packet = {:?}", packet);
                }
                Err(e) => {
                    eprintln!("Error receiving packet: {}", e);
                }
            }
        }
    });

    let packets = vec![
        ClientPacket::Connect {
            player_name: "test".to_string(),
        },
        ClientPacket::Input(ClientInput {
            prediected_time: SystemTime::now(),
            sequence: 0,
            move_direction: Vec2::new(1.0, 0.0),
            shoot: false,
        }),
        ClientPacket::Input(ClientInput {
            prediected_time: SystemTime::now(),
            sequence: 1,
            move_direction: Vec2::new(1.0, 0.0),
            shoot: false,
        }),
        ClientPacket::Input(ClientInput {
            prediected_time: SystemTime::now(),
            sequence: 2,
            move_direction: Vec2::new(1.0, 0.0),
            shoot: false,
        }),
        ClientPacket::Input(ClientInput {
            prediected_time: SystemTime::now(),
            sequence: 3,
            move_direction: Vec2::new(1.0, 0.0),
            shoot: false,
        }),
        ClientPacket::Input(ClientInput {
            prediected_time: SystemTime::now(),
            sequence: 4,
            move_direction: Vec2::new(1.0, 0.0),
            shoot: false,
        }),
        ClientPacket::Disconnect,
    ];

    for packet in packets {
        send_packet(&socket, packet).await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    Ok(())
}

async fn send_packet(socket: &UdpSocket, packet: ClientPacket) -> anyhow::Result<()> {
    let packet_encoded = bincode::serde::encode_to_vec(packet, bincode::config::standard())?;

    socket.send_to(&packet_encoded, SERVER_ADDR).await?;

    Ok(())
}
