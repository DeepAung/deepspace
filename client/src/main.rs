use bevy::math::ops::atan;
use bevy::prelude::*;
use bevy_simple_text_input::{
    TextInput, TextInputPlugin, TextInputSubmitMessage, TextInputSystem, TextInputTextColor,
    TextInputTextFont, TextInputValue,
};
// Using crossbeam_channel instead of std as std `Receiver` is `!Sync`
use crossbeam::channel::{Receiver, bounded};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};

use game_client::GameClient;
use shared::{BulletId, PlayerId, TIME_SYNC_DURATION};

use crate::game_client::RenderState;

mod game_client;
mod mod_test;
mod plugins;

// --- Constants ---
const SPACESHIP_SHAPE: Triangle2d = Triangle2d::new(
    Vec2::new(0.0, 20.0),
    Vec2::new(-15.0, -20.0),
    Vec2::new(15.0, -20.0),
);

const BULLET_SHAPE: Rectangle = Rectangle::new(2.0, 5.0);

const MY_PLAYER_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);
const OTHER_PLAYER_COLOR: Color = Color::srgb(1.0, 0.0, 0.0);
const MY_BULLET_COLOR: Color = Color::srgb(0.0, 0.0, 1.0);
const OTHER_BULLET_COLOR: Color = Color::srgb(1.0, 0.0, 0.0);

const PLAYER_LAYER: f32 = 10.0;
const BULLET_LAYER: f32 = 5.0;

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

// --- States ---
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum GameState {
    #[default]
    Welcome,
    InGame,
}

// --- Components ---

/// A "Tag" component to identify the UI root node so we can despawn it later

#[derive(Component)]
struct WelcomeScreenRoot;

#[derive(Component)]
struct GameScreenRoot;

#[derive(Component)]
struct StartButton;

#[derive(Component)]
struct PlayerNameTextInput;

#[derive(Resource, Deref)]
struct StreamReceiver(Receiver<RenderState>);

#[derive(Component)]
struct LocalPlayer;

#[derive(Component)]
struct RemotePlayer;

#[derive(Component)]
struct Player {
    id: PlayerId,
}

#[derive(Component)]
struct Bullet {
    id: BulletId,
}

// --- Main ---

const BORDER_COLOR_ACTIVE: Color = Color::srgb(0.75, 0.52, 0.99);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const BACKGROUND_COLOR: Color = Color::srgb(0.15, 0.15, 0.15);

fn main() -> anyhow::Result<()> {
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
        .add_plugins(TextInputPlugin)
        .init_state::<GameState>()
        .insert_resource(StreamReceiver(rx))
        .insert_resource(NetworkClient::new(client_socket, game_client))
        .insert_resource(Time::<Fixed>::from_duration(TIME_SYNC_DURATION))
        .add_systems(Startup, setup_camera)
        .add_systems(OnEnter(GameState::Welcome), setup_welcome_screen)
        .add_systems(
            Update,
            (
                listener.after(TextInputSystem),
                button_system,
                check_connection_status,
            )
                .run_if(in_state(GameState::Welcome)),
        )
        .add_systems(OnExit(GameState::Welcome), teardown_welcome_screen)
        .add_systems(OnEnter(GameState::InGame), setup_game_screen)
        .add_systems(
            Update,
            (handle_input, render_game).run_if(in_state(GameState::InGame)),
        )
        .add_systems(
            FixedUpdate,
            time_sync_system.run_if(in_state(GameState::InGame)),
        )
        .add_systems(OnExit(GameState::InGame), teardown_game_screen)
        .run();

    Ok(())
}

// --- Systems ---

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn setup_welcome_screen(mut commands: Commands) {
    commands
        .spawn(WelcomeScreenRoot)
        .insert(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Vh(5.0),
            ..Default::default()
        })
        .with_children(|parent| {
            parent.spawn(Text::new("Enter you name"));

            parent.spawn(PlayerNameTextInput).insert((
                Node {
                    width: Val::Px(200.0),
                    border: UiRect::all(Val::Px(5.0)),
                    padding: UiRect::all(Val::Px(5.0)),
                    ..default()
                },
                BorderColor::all(BORDER_COLOR_ACTIVE),
                BackgroundColor(BACKGROUND_COLOR),
                TextInput,
                TextInputValue(String::new()),
                TextInputTextFont(TextFont {
                    font_size: 34.,
                    ..default()
                }),
                TextInputTextColor(TextColor(TEXT_COLOR)),
            ));

            parent.spawn(StartButton).insert((
                Button,
                Node {
                    width: px(150),
                    height: px(65),
                    border: UiRect::all(px(5)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(Color::WHITE),
                BorderRadius::MAX,
                BackgroundColor(Color::BLACK),
                children![(
                    Text::new("Button"),
                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    TextShadow::default(),
                )],
            ));
        });
}

fn teardown_welcome_screen(mut commands: Commands, query: Query<Entity, With<WelcomeScreenRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn listener(mut events: MessageReader<TextInputSubmitMessage>, network_client: Res<NetworkClient>) {
    for event in events.read() {
        let player_name = &event.value;
        info!("'Enter' key submitted: {}", player_name);

        // Lock the mutex to access the GameClient
        if let Ok(mut client) = network_client.client.lock() {
            try_connect(&mut client, &network_client.socket, player_name.clone());
        }
    }
}

fn button_system(
    // 1. Query the button interaction
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<StartButton>)>,
    // 2. Query the TextInput value. We use `With<PlayerNameTextInput>` to find the specific box.
    mut text_input_query: Query<&mut TextInputValue, With<PlayerNameTextInput>>,
    network_client: Res<NetworkClient>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            // Get the text from the input field
            if let Ok(mut input) = text_input_query.single_mut() {
                let player_name = std::mem::take(&mut input.0);

                info!("Start Button clicked: {}", player_name);

                // Same connection logic as the listener
                if let Ok(mut client) = network_client.client.lock() {
                    try_connect(&mut client, &network_client.socket, player_name.clone());
                }
            } else {
                warn!("Start button clicked, but could not find the text input!");
            }
        }
    }
}

fn try_connect(client: &mut GameClient, socket: &UdpSocket, player_name: String) {
    if client.connected() {
        return;
    }

    client.connect(&socket, player_name).unwrap();
}

fn check_connection_status(
    network_client: Res<NetworkClient>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // TODO: this might deadlock
    println!("check_connection_status");
    if let Ok(client) = network_client.client.lock() {
        if client.connected() {
            info!("Connection successful! Switching to Game state.");
            next_state.set(GameState::InGame);
        }
    }
}

fn setup_game_screen(
    mut commands: Commands,
    // mut meshes: ResMut<Assets<Mesh>>,
    // mut materials: ResMut<Assets<ColorMaterial>>,
    // network_client: Res<NetworkClient>,
) {
    info!("GAME STARTED!");
    commands.spawn(GameScreenRoot);

    // TODO: setup UI like scoreboard, etc.
}

fn teardown_game_screen(mut commands: Commands, query: Query<Entity, With<GameScreenRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn handle_input(mut commands: Commands, query: Query<&LocalPlayer>) {}

fn render_game(
    mut commands: Commands,
    receiver: Res<StreamReceiver>,

    game_root_query: Query<Entity, With<GameScreenRoot>>,
    mut local_player_query: Query<(Entity, &mut Transform), With<LocalPlayer>>,
    mut remote_players_query: Query<
        (Entity, &Player, &mut Transform),
        (With<RemotePlayer>, Without<LocalPlayer>),
    >,
    mut bullets_query: Query<
        (Entity, &Bullet, &mut Transform),
        (With<Bullet>, Without<LocalPlayer>, Without<RemotePlayer>),
    >,

    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Some(state) = receiver.try_iter().last() else {
        println!("state from rx");
        return;
    };

    info!("get render state {:?}", state);

    let Ok(game_root) = game_root_query.single() else {
        println!("no game_root");
        return;
    };

    // --- LOCAL PLAYER ---
    let Some(local_player_state) = state.local_player else {
        println!("no local_player");
        return;
    };

    let offset_x = local_player_state.position.x;
    let offset_y = local_player_state.position.y;

    match local_player_query.single_mut() {
        Ok((_, mut transform)) => {
            let angle = {
                let vec = local_player_state.velocity.normalized();
                atan(vec.y / vec.x)
            };
            transform.rotate_z(angle);
        }
        Err(_) => {
            commands
                .spawn((
                    Player {
                        id: local_player_state.id,
                    },
                    LocalPlayer,
                    Mesh2d(meshes.add(SPACESHIP_SHAPE)),
                    MeshMaterial2d(materials.add(MY_PLAYER_COLOR)),
                    Transform::from_xyz(0.0, 0.0, PLAYER_LAYER),
                ))
                .set_parent_in_place(game_root);
        }
    }

    // --- REMOTE PLAYERS ---
    // 1. Build a map of incoming data [PlayerId -> Position]
    let mut remote_player_states: HashMap<PlayerId, Vec2> = state
        .other_players
        .iter()
        .map(|p| (p.id, Vec2::new(p.position.x, p.position.y)))
        .collect();

    // 2. Update or Despawn existing entities
    for (entity, player, mut transform) in remote_players_query.iter_mut() {
        if let Some(&pos) = remote_player_states.get(&player.id) {
            // Update position
            transform.translation = Vec3::new(pos.x, pos.y, PLAYER_LAYER);
            // Remove from map so we know we processed it
            remote_player_states.remove(&player.id);
        } else {
            commands.entity(entity).despawn();
        }
    }

    // 3. Spawn new entities (whatever is left in the map)
    for (id, pos) in remote_player_states {
        commands
            .spawn((
                Player { id },
                RemotePlayer,
                Mesh2d(meshes.add(SPACESHIP_SHAPE)),
                MeshMaterial2d(materials.add(OTHER_PLAYER_COLOR)),
                Transform::from_xyz(pos.x - offset_x, pos.y - offset_y, PLAYER_LAYER),
            ))
            .set_parent_in_place(game_root);
    }

    // --- BULLETS ---
    // 1. Build map [BulletId -> (Position, OwnerId)]
    let mut bullet_states: HashMap<BulletId, (Vec2, PlayerId)> = state
        .bullets
        .iter()
        .map(|b| (b.id, (Vec2::new(b.position.x, b.position.y), b.owner_id)))
        .collect();

    // 2. Update or Despawn existing
    for (entity, bullet, mut transform) in bullets_query.iter_mut() {
        if let Some(&(pos, _)) = bullet_states.get(&bullet.id) {
            transform.translation = Vec3::new(pos.x, pos.y, BULLET_LAYER);
            bullet_states.remove(&bullet.id);
        } else {
            commands.entity(entity).despawn();
        }
    }

    // 3. Spawn new
    for (id, (pos, owner_id)) in bullet_states {
        // Determine color based on owner
        let color = if owner_id == local_player_state.id {
            MY_BULLET_COLOR
        } else {
            OTHER_BULLET_COLOR
        };

        commands
            .spawn((
                Bullet { id },
                Mesh2d(meshes.add(BULLET_SHAPE)),
                MeshMaterial2d(materials.add(color)),
                Transform::from_xyz(pos.x, pos.y, BULLET_LAYER),
            ))
            .set_parent_in_place(game_root);
    }
}

fn time_sync_system(_time: Res<Time<Fixed>>, network_client: Res<NetworkClient>) {
    println!("run time_sync_system");
    if let Ok(client) = network_client.client.lock() {
        if !client.connected() {
            return;
        }
        println!("ok time_sync_system");

        if let Err(e) = client.time_sync(&network_client.socket) {
            // When closing the app, this error is expected. We just log it.
            warn!("Time sync failed: {:?}", e);
        }
    }
}
