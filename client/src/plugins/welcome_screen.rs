use std::net::UdpSocket;

use bevy::prelude::*;
use bevy_simple_text_input::{
    TextInput, TextInputPlugin, TextInputSubmitMessage, TextInputSystem, TextInputTextColor,
    TextInputTextFont, TextInputValue,
};

use crate::{
    game_client::GameClient,
    plugins::{GameState, NetworkClient},
};

pub struct WelcomeScreenPlugin;

impl Plugin for WelcomeScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TextInputPlugin)
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
            .add_systems(OnExit(GameState::Welcome), teardown_welcome_screen);
    }
}

// --- Constants ---
const BORDER_COLOR_ACTIVE: Color = Color::srgb(0.75, 0.52, 0.99);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const BACKGROUND_COLOR: Color = Color::srgb(0.15, 0.15, 0.15);

// --- Components ---
#[derive(Component)]
struct WelcomeScreenRoot;

#[derive(Component)]
struct StartButton;

#[derive(Component)]
struct PlayerNameTextInput;

// --- Systems ---
fn setup_welcome_screen(mut commands: Commands) {
    commands
        .spawn(WelcomeScreenRoot)
        .insert(Camera2d)
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
