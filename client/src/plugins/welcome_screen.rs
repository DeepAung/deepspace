use bevy::{prelude::*, sprite_render::Material2dPlugin};
use bevy_simple_text_input::{
    TextInput, TextInputPlugin, TextInputSubmitMessage, TextInputSystem, TextInputTextColor,
    TextInputTextFont, TextInputValue,
};

use crate::{
    assets::{FONT_PATH, SpaceBackgroundMaterial},
    plugins::{GameState, TARGET_HEIGHT, TARGET_WIDTH, network::NetworkClient},
};

pub struct WelcomeScreenPlugin;

impl Plugin for WelcomeScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<SpaceBackgroundMaterial>::default())
            .add_plugins(TextInputPlugin)
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

// --- Components ---
#[derive(Component)]
struct WelcomeObject;

#[derive(Component)]
struct StartButton;

#[derive(Component)]
struct PlayerNameTextInput;

// --- Systems ---
fn setup_welcome_screen(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut space_materials: ResMut<Assets<SpaceBackgroundMaterial>>,
    server: Res<AssetServer>,
) {
    // TODO: find out why background didn't render when go from Ingame to Welcome
    // Background
    commands.spawn((
        WelcomeObject,
        Mesh2d(meshes.add(Rectangle::new(TARGET_WIDTH, TARGET_HEIGHT))),
        MeshMaterial2d(space_materials.add(SpaceBackgroundMaterial::default())),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    commands.spawn((
        WelcomeObject,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(16.0),
            ..default()
        },
        children![
            (
                Text::new("DEEPSPACE"),
                TextFont {
                    font: server.load(FONT_PATH),
                    font_size: 72.0,
                    ..default()
                },
                Node {
                    margin: UiRect::bottom(Val::Px(100.0)),
                    ..default()
                }
            ),
            (
                Text::new("Enter you name"),
                TextFont {
                    font: server.load(FONT_PATH),
                    ..default()
                }
            ),
            (
                PlayerNameTextInput,
                Node {
                    width: Val::Px(200.0),
                    border: UiRect::all(Val::Px(4.0)),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BorderColor::all(Color::WHITE),
                BorderRadius::MAX,
                BackgroundColor(Color::BLACK),
                TextInput,
                TextInputValue(String::new()),
                TextInputTextFont(TextFont {
                    font_size: 24.0,
                    ..default()
                }),
                TextInputTextColor(TextColor(Color::WHITE)),
            ),
            (
                StartButton,
                Button,
                Node {
                    width: Val::Px(150.0),
                    height: Val::Px(65.0),
                    border: UiRect::all(Val::Px(4.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(Color::WHITE),
                BorderRadius::MAX,
                BackgroundColor(Color::BLACK),
                children![(
                    Text::new("Start"),
                    TextFont {
                        font: server.load(FONT_PATH),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextShadow::default(),
                )],
            )
        ],
    ));
}

fn teardown_welcome_screen(mut commands: Commands, query: Query<Entity, With<WelcomeObject>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn listener(
    mut events: MessageReader<TextInputSubmitMessage>,
    mut network_client: ResMut<NetworkClient>,
) {
    for event in events.read() {
        let player_name = &event.value;
        info!("'Enter' key submitted: {}", player_name);

        try_connect(&mut network_client, player_name.clone());
    }
}

fn button_system(
    // 1. Query the button interaction
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<StartButton>)>,
    // 2. Query the TextInput value. We use `With<PlayerNameTextInput>` to find the specific box.
    mut text_input_query: Query<&mut TextInputValue, With<PlayerNameTextInput>>,
    mut network_client: ResMut<NetworkClient>,
) {
    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            // Get the text from the input field
            if let Ok(mut input) = text_input_query.single_mut() {
                let player_name = std::mem::take(&mut input.0);

                info!("Start Button clicked: {}", player_name);

                // Same connection logic as the listener
                try_connect(&mut network_client, player_name.clone());
            } else {
                warn!("Start button clicked, but could not find the text input!");
            }
        }
    }
}

fn try_connect(network_client: &mut NetworkClient, player_name: String) {
    if network_client.connected() {
        return;
    }

    network_client.connect(player_name).unwrap();
}

fn check_connection_status(
    network_client: Res<NetworkClient>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if network_client.connected() {
        info!("Connection successful! Switching to Game state.");
        next_state.set(GameState::InGame);
    }
}
