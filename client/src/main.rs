use bevy::ecs::schedule::And;
use bevy::prelude::*;
use bevy_simple_text_input::{
    TextInput, TextInputPlugin, TextInputSubmitMessage, TextInputSystem, TextInputTextColor,
    TextInputTextFont, TextInputValue,
};
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};

use game_client::GameClient;
use shared::{BulletId, PlayerId, TIME_SYNC_DURATION};

mod game_client;
mod mod_test;
mod plugins;

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
struct StartButton;

#[derive(Component)]
struct PlayerNameTextInput;

#[derive(Component)]
struct LocalPlayer {
    id: PlayerId,
}

#[derive(Component)]
struct RemotePlayer {
    id: PlayerId,
}

#[derive(Component)]
struct RemoteBullet {
    id: BulletId,
}

// --- Main ---

const BORDER_COLOR_ACTIVE: Color = Color::srgb(0.75, 0.52, 0.99);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const BACKGROUND_COLOR: Color = Color::srgb(0.15, 0.15, 0.15);

fn main() -> anyhow::Result<()> {
    let server_addr: SocketAddr = "127.0.0.1:8080".parse().expect("Invalid address");
    let game_client = Arc::new(Mutex::new(GameClient::new(server_addr)));

    let client_socket = Arc::new(UdpSocket::bind("0.0.0.0:0")?);
    println!("UDP client bound to: {}", client_socket.local_addr()?);

    let loop_game_client = Arc::clone(&game_client);
    let loop_client_socket = Arc::clone(&client_socket);
    std::thread::spawn(|| {
        GameClient::recv_loop(loop_game_client, loop_client_socket);
    });

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TextInputPlugin)
        .init_state::<GameState>()
        // .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1))) // Dark background
        .insert_resource(NetworkClient::new(client_socket, game_client))
        .insert_resource(Time::<Fixed>::from_duration(TIME_SYNC_DURATION))
        .add_systems(Startup, setup_camera)
        .add_systems(OnEnter(GameState::Welcome), setup_welcome_screen)
        .add_systems(
            Update,
            (listener.after(TextInputSystem), button_system).run_if(in_state(GameState::Welcome)),
        )
        .add_systems(OnExit(GameState::Welcome), teardown_welcome_screen)
        .add_systems(OnEnter(GameState::InGame), setup_game_screen)
        // .add_systems(Update, (handle_input, render_graphics))
        .add_systems(FixedUpdate, time_sync_system)
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

fn setup_game_screen(mut commands: Commands) {
    debug!("GAME STARTED!");
    commands.spawn(Text::new("Game World Loaded!"));
}

fn listener(
    mut events: MessageReader<TextInputSubmitMessage>,
    network_client: Res<NetworkClient>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for event in events.read() {
        let player_name = &event.value;
        debug!("'Enter' key submitted: {}", player_name);

        // Lock the mutex to access the GameClient
        if let Ok(mut client) = network_client.client.lock() {
            connect_and_set_state(
                &mut client,
                &network_client.socket,
                player_name.clone(),
                &mut next_state,
            );
        }
    }
}

fn button_system(
    // 1. Query the button interaction
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<StartButton>)>,
    // 2. Query the TextInput value. We use `With<PlayerNameTextInput>` to find the specific box.
    mut text_input_query: Query<&mut TextInputValue, With<PlayerNameTextInput>>,
    network_client: Res<NetworkClient>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            // Get the text from the input field
            if let Ok(mut input) = text_input_query.single_mut() {
                let player_name = std::mem::take(&mut input.0);

                debug!("Start Button clicked: {}", player_name);

                // Same connection logic as the listener
                if let Ok(mut client) = network_client.client.lock() {
                    connect_and_set_state(
                        &mut client,
                        &network_client.socket,
                        player_name.clone(),
                        &mut next_state,
                    );
                }
            } else {
                warn!("Start button clicked, but could not find the text input!");
            }
        }
    }
}

fn connect_and_set_state(
    client: &mut GameClient,
    socket: &UdpSocket,
    player_name: String,
    next_state: &mut ResMut<NextState<GameState>>,
) {
    if client.connected() {
        return;
    }

    client.connect(&socket, player_name).unwrap();
    next_state.set(GameState::InGame);
}

fn time_sync_system(_time: Res<Time<Fixed>>, network_client: Res<NetworkClient>) {
    if let Ok(client) = network_client.client.lock() {
        if client.connected() {
            client.time_sync(&network_client.socket).unwrap();
        }
    }
}
