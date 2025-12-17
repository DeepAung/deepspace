mod game_screen;
mod welcome_screen;

use bevy::prelude::*;
// Using crossbeam_channel instead of std as std `Receiver` is `!Sync`
use crossbeam::channel::{Receiver, bounded};
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};

use crate::game_client::{GameClient, RenderState};
use crate::plugins::game_screen::GameScreenPlugin;
use crate::plugins::welcome_screen::WelcomeScreenPlugin;

// --- Resources ---
#[derive(Resource, Clone)]
struct NetworkClient {
    pub socket: Arc<UdpSocket>,
    pub client: Arc<Mutex<GameClient>>,
}

impl NetworkClient {
    pub fn new(socket: Arc<UdpSocket>, client: Arc<Mutex<GameClient>>) -> Self {
        Self { socket, client }
    }
}

#[derive(Resource, Deref)]
struct StreamReceiver(Receiver<RenderState>);

// --- States ---
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum GameState {
    #[default]
    Welcome,
    InGame,
}

pub fn init_game() -> anyhow::Result<()> {
    let (tx, rx) = bounded::<RenderState>(1);

    let server_addr: SocketAddr = "127.0.0.1:8080".parse().expect("Invalid address");
    let game_client = Arc::new(Mutex::new(GameClient::new(server_addr)));

    let client_socket = Arc::new(UdpSocket::bind("0.0.0.0:0")?);
    println!("UDP client bound to: {}", client_socket.local_addr()?);

    let loop_game_client = Arc::clone(&game_client);
    let loop_client_socket = Arc::clone(&client_socket);
    std::thread::spawn(|| {
        GameClient::recv_loop(loop_game_client, loop_client_socket, tx);
    });

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WelcomeScreenPlugin)
        .add_plugins(GameScreenPlugin)
        .init_state::<GameState>()
        .insert_resource(StreamReceiver(rx))
        .insert_resource(NetworkClient::new(client_socket, game_client))
        .run();

    Ok(())
}
