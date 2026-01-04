use std::{
    net::{SocketAddr, UdpSocket},
    sync::Arc,
    time::SystemTime,
};

use bevy::{prelude::*, time::common_conditions::on_timer};
use crossbeam::channel::{Receiver, bounded}; // Using crossbeam_channel instead of std as std `Receiver` is `!Sync`
use shared::{ServerPacket, TIME_SYNC_DURATION};

use crate::{game_client::GameClient, plugins::GameState};

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        let server_addr: SocketAddr = "127.0.0.1:8080".parse().expect("Invalid address");

        let client_socket =
            Arc::new(UdpSocket::bind("0.0.0.0:0").expect("fail to create UDP socket"));

        client_socket
            .set_nonblocking(true)
            .expect("Cannot set UDP socket as non-blocking");

        println!(
            "UDP client bound to: {}",
            client_socket
                .local_addr()
                .expect("cannot get UDP socket address")
        );

        let (tx, rx) = bounded::<(ServerPacket, SystemTime)>(1);

        let loop_client_socket = Arc::clone(&client_socket);
        std::thread::spawn(move || {
            GameClient::recv_loop(loop_client_socket, tx);
        });

        app.insert_resource(StreamReceiver(rx))
            .insert_resource(NetworkClient(GameClient::new(server_addr, client_socket)))
            .add_systems(Update, handle_packet)
            .add_systems(
                FixedUpdate,
                time_sync_system
                    .run_if(in_state(GameState::InGame).and(on_timer(TIME_SYNC_DURATION))),
            );
    }
}

// --- Resources ---
#[derive(Resource, Deref, DerefMut)]
pub struct NetworkClient(pub GameClient);

#[derive(Resource, Deref)]
struct StreamReceiver(Receiver<(ServerPacket, SystemTime)>);

// --- Systems ---

fn handle_packet(receiver: Res<StreamReceiver>, mut network_client: ResMut<NetworkClient>) {
    for (packet, client_recv_time) in receiver.try_iter() {
        network_client.handle_packet(packet, client_recv_time);
    }
}

fn time_sync_system(network_client: Res<NetworkClient>) {
    if !network_client.connected() {
        return;
    }

    debug!("run time sync");
    if let Err(e) = network_client.time_sync() {
        // When closing the app, this error is expected. We just log it.
        warn!("Time sync failed: {:?}", e);
    }
}
