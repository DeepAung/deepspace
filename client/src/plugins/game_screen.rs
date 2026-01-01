use bevy::ecs::query::QuerySingleError;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{Material2d, Material2dPlugin};
use bevy::time::common_conditions::on_timer;
use bevy::window::PrimaryWindow;
use shared::{
    BulletId, BulletState, LifeState, MoveDirection, PlayerId, PlayerState, ScoreEntry,
    TICK_DURATION, WORLD_HEIGHT, WORLD_WIDTH,
};
use std::collections::HashMap;
use std::f32::consts::PI;

use crate::game_client::RenderState;
use crate::plugins::{GameState, network::NetworkClient};

pub struct GameScreenPlugin;

// TODO: add exit button
// TODO: disconnect on close window
// TODO: add font

impl Plugin for GameScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<GridMaterial>::default())
            .insert_resource(RenderStateResource(None))
            .add_systems(OnEnter(GameState::InGame), setup_game_screen)
            .add_systems(OnExit(GameState::InGame), teardown_game_screen)
            .add_systems(
                Update,
                (
                    handle_input.run_if(on_timer(TICK_DURATION)),
                    update_camera,
                    render_respawn_popup,
                )
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (
                    update_render_state_resource,
                    render_local_player,
                    render_remote_players,
                    render_bullets,
                    update_health_bars_position,
                    update_health_bars_value,
                    update_scoreboard,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

// --- Constants ---

const SPACESHIP_SHAPE: Triangle2d = Triangle2d::new(
    Vec2::new(0.0, 20.0),
    Vec2::new(-15.0, -20.0),
    Vec2::new(15.0, -20.0),
);

const BULLET_SHAPE: Rectangle = Rectangle::new(5.0, 8.0);

const HEALTH_BAR_SIZE: Vec2 = Vec2::new(60.0, 5.0);

const MY_PLAYER_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);
const OTHER_PLAYER_COLOR: Color = Color::srgb(1.0, 0.0, 0.0);
const MY_BULLET_COLOR: Color = Color::srgb(0.0, 0.0, 1.0);
const OTHER_BULLET_COLOR: Color = Color::srgb(1.0, 0.0, 0.0);
const POPUP_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.6);
const HEALTH_BAR_BG_COLOR: Color = Color::srgb(0.2, 0.0, 0.0);
const MY_HEALTH_BAR_FG_COLOR: Color = Color::srgb(0.0, 1.0, 0.0);
const OTHER_HEALTH_BAR_FG_COLOR: Color = Color::srgb(1.0, 0.0, 0.0);

const BACKGROUND_LAYER: f32 = 0.0;
const BULLET_LAYER: f32 = 1.0;
const PLAYER_LAYER: f32 = 2.0;
const PLAYER_INFO_LAYER: f32 = 3.0;

// --- Resources ---

#[derive(Resource, Deref)]
pub struct RenderStateResource(pub Option<RenderState>);

// --- Components ---

#[derive(Component)]
struct InGameObject;

#[derive(Component)]
struct RespawnPopup;

#[derive(Component)]
struct RespawnPopupText;

#[derive(Component)]
struct LocalPlayer;

#[derive(Component)]
struct RemotePlayer;

enum PlayerMarker {
    Local,
    Remote,
}

#[derive(Component)]
struct Player {
    id: PlayerId,
}

#[derive(Component)]
struct Bullet {
    id: BulletId,
}

#[derive(Component)]
struct Health {
    current: i32,
    max: i32,
}

#[derive(Component)]
struct HealthBarValue {
    player_entity: Entity,
}

#[derive(Component)]
struct Billboard {
    offset: Vec2,
}

#[derive(Component)]
pub struct Scoreboard(Vec<ScoreEntry>);

// --- Materials ---

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct GridMaterial {
    #[uniform(0)]
    color: LinearRgba,
    #[uniform(1)]
    bg_color: LinearRgba,
    #[uniform(2)]
    grid_size: f32, // How many grid cells across the image
    #[uniform(3)]
    thickness: f32, // Thickness of the lines (0.0 to 1.0 relative to cell size)
}

impl Default for GridMaterial {
    fn default() -> Self {
        const LINE_COLOR: LinearRgba = LinearRgba::new(0.03, 0.03, 0.03, 1.0);
        const BG_COLOR: LinearRgba = LinearRgba::new(0.0, 0.0, 0.0, 1.0);
        const GRID_SIZE: f32 = 100.0;
        const THICKNESS: f32 = 0.05;

        Self {
            color: LINE_COLOR,
            bg_color: BG_COLOR,
            grid_size: GRID_SIZE,
            thickness: THICKNESS,
        }
    }
}

impl Material2d for GridMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/grid_background.wgsl".into()
    }
}

// --- Systems ---

fn setup_game_screen(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut grid_materials: ResMut<Assets<GridMaterial>>,
) {
    // Background
    commands.spawn((
        InGameObject,
        Mesh2d(meshes.add(Rectangle::new(WORLD_WIDTH, WORLD_HEIGHT))),
        MeshMaterial2d(grid_materials.add(GridMaterial::default())),
        Transform::from_xyz(0.0, 0.0, BACKGROUND_LAYER),
    ));

    // Respawn Popup
    commands.spawn((
        InGameObject,
        RespawnPopup,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(POPUP_COLOR),
        children![(Text::new("Respawn in X"), RespawnPopupText)],
    ));

    // Scoreboard
    commands.spawn((
        InGameObject,
        Scoreboard(Vec::new()),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(32.0),
            right: Val::Px(32.0),

            width: Val::Px(300.0),
            height: Val::Auto,

            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Center,
            ..default()
        },
    ));
}

fn teardown_game_screen(mut commands: Commands, query: Query<Entity, With<InGameObject>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn update_camera(
    mut camera: Single<&mut Transform, (With<Camera2d>, Without<LocalPlayer>)>,
    local_player: Single<&Transform, (With<LocalPlayer>, Without<Camera2d>)>,
    time: Res<Time>,
) {
    const CAMERA_DECAY_RATE: f32 = 1.0;

    let Vec3 { x, y, .. } = local_player.translation;
    let direction = Vec3::new(x, y, camera.translation.z);

    camera
        .translation
        .smooth_nudge(&direction, CAMERA_DECAY_RATE, time.delta_secs());
}

fn handle_input(
    kb_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut network_client: ResMut<NetworkClient>,
    state: Res<RenderStateResource>,

    mut last_rotation: Local<f32>,
) {
    // Ignore input if player is dead
    if let Some(state) = &state.0 {
        if let Some(local_player) = &state.local_player {
            if matches!(local_player.life, LifeState::Dead { respawn_time: _ }) {
                return;
            }
        }
    }

    let rotation = match window.cursor_position() {
        Some(mouse_position) => {
            let center_position = Vec2::new(window.width() / 2.0, window.height() / 2.0);

            let mut direction = mouse_position - center_position;
            direction.y = -direction.y; // Flip y axis so that Y increase when go upward

            let angle = direction.to_angle();

            *last_rotation = angle;

            angle
        }
        None => *last_rotation,
    };

    let shoot = mouse_input.just_pressed(MouseButton::Left);

    let move_direction = if kb_input.pressed(KeyCode::KeyW) {
        MoveDirection::Forward
    } else if kb_input.pressed(KeyCode::KeyS) {
        MoveDirection::Backward
    } else {
        MoveDirection::None
    };

    network_client
        .send_input(move_direction, rotation, shoot)
        .unwrap();
}

fn update_render_state_resource(
    network_client: Res<NetworkClient>,
    mut render_state_resource: ResMut<RenderStateResource>,
) {
    let render_state = network_client.get_render_state();
    render_state_resource.0 = Some(render_state);
}

fn render_local_player(
    mut commands: Commands,
    state: Res<RenderStateResource>,

    mut camera: Single<&mut Transform, With<Camera2d>>,
    mut local_player_query: Query<
        (&mut Health, &mut Transform, &mut Visibility),
        (With<LocalPlayer>, Without<Camera2d>),
    >,

    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Some(state) = &state.0 else {
        return;
    };

    let Some(local_player_state) = &state.local_player else {
        return;
    };

    match local_player_query.single_mut() {
        Ok((mut health, mut transform, mut visibility)) => {
            update_player(
                local_player_state,
                &mut health,
                &mut transform,
                &mut visibility,
            );
        }
        Err(QuerySingleError::NoEntities(_)) => {
            create_player(
                &mut commands,
                local_player_state,
                PlayerMarker::Local,
                &mut meshes,
                &mut materials,
            );

            camera.translation.x = local_player_state.position.x;
            camera.translation.y = local_player_state.position.y;
        }
        Err(QuerySingleError::MultipleEntities(_)) => {
            panic!("There are multiple instances of LocalPlayer");
        }
    };
}

fn render_remote_players(
    mut commands: Commands,
    state: Res<RenderStateResource>,

    mut remote_players_query: Query<
        (
            Entity,
            &Player,
            &mut Health,
            &mut Transform,
            &mut Visibility,
        ),
        With<RemotePlayer>,
    >,

    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Some(state) = &state.0 else {
        return;
    };

    // 1. Build a map of incoming data [PlayerId -> Position]
    let mut remote_player_states: HashMap<PlayerId, &PlayerState> =
        state.other_players.iter().map(|p| (p.id, p)).collect();

    // 2. Update or Despawn existing entities
    for (entity, player, mut health, mut transform, mut visibility) in
        remote_players_query.iter_mut()
    {
        if let Some(&player_state) = remote_player_states.get(&player.id) {
            update_player(player_state, &mut health, &mut transform, &mut visibility);

            // Remove from map so we know we processed it
            remote_player_states.remove(&player.id);
        } else {
            commands.entity(entity).despawn();
        }
    }

    // 3. Spawn new entities (whatever is left in the map)
    for (_, player_state) in remote_player_states {
        create_player(
            &mut commands,
            player_state,
            PlayerMarker::Remote,
            &mut meshes,
            &mut materials,
        );
    }
}

fn render_bullets(
    mut commands: Commands,
    state: Res<RenderStateResource>,

    mut bullets_query: Query<(Entity, &Bullet, &mut Transform), With<Bullet>>,

    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Some(state) = &state.0 else {
        return;
    };

    let Some(local_player_state) = &state.local_player else {
        return;
    };

    // 1. Build map [BulletId -> (Position, OwnerId)]
    let mut bullet_states: HashMap<BulletId, &BulletState> =
        state.bullets.iter().map(|b| (b.id, b)).collect();

    // 2. Update or Despawn existing
    for (entity, bullet, mut transform) in bullets_query.iter_mut() {
        if let Some(bullet_state) = bullet_states.get(&bullet.id) {
            update_bullet(bullet_state, &mut transform);

            bullet_states.remove(&bullet.id);
        } else {
            commands.entity(entity).despawn();
        }
    }

    // 3. Spawn new
    for (_, bullet_state) in bullet_states {
        create_bullet(
            &mut commands,
            local_player_state.id,
            bullet_state,
            &mut meshes,
            &mut materials,
        );
    }
}

fn update_health_bars_position(
    player_query: Query<(&Transform, &Children), (With<Player>, Changed<Transform>)>,
    mut child_query: Query<(&mut Transform, &Billboard), Without<Player>>,
) {
    for (parent_transform, children) in player_query.iter() {
        let inv_rotation = parent_transform.rotation.inverse();

        for child in children.iter() {
            if let Ok((mut child_transform, billboard)) = child_query.get_mut(child) {
                child_transform.rotation = inv_rotation;

                let z = child_transform.translation.z;
                child_transform.translation =
                    inv_rotation * Vec3::new(billboard.offset.x, billboard.offset.y, 0.0);
                child_transform.translation.z = z;
            }
        }
    }
}

fn update_health_bars_value(
    mut health_bar_value_query: Query<
        (&HealthBarValue, &mut Transform, &mut Billboard),
        With<HealthBarValue>,
    >,
    player_query: Query<&Health, (With<Player>, Without<HealthBarValue>)>,
) {
    for (health_bar_value, mut transform, mut billboard) in health_bar_value_query.iter_mut() {
        if let Ok(health) = player_query.get(health_bar_value.player_entity) {
            let percent = (health.current as f32 / health.max as f32).clamp(0.0, 1.0);

            // Only update if changed (optimization)
            if (transform.scale.x - percent).abs() > f32::EPSILON {
                transform.scale.x = percent;

                // Visual Fix: Mesh2d scales from the Center.
                // To make it look like it's shrinking from Right-to-Left (Anchored Left),
                // we must shift the position slightly as it gets smaller.
                // Formula: shift = (OriginalWidth * Scale - OriginalWidth) / 2.0
                let shift = (HEALTH_BAR_SIZE.x * percent - HEALTH_BAR_SIZE.x) / 2.0;
                billboard.offset.x = shift; // translation will reflect the billboard offset changed
            }
        }
    }
}

fn update_scoreboard(
    mut commands: Commands,
    state: Res<RenderStateResource>,
    scoreboard_entity: Single<Entity, With<Scoreboard>>,
) {
    let Some(state) = &state.0 else {
        return;
    };

    commands.entity(*scoreboard_entity).despawn_children();

    let score_entries = state.scoreboard.clone();

    // 4. Spawn new rows
    commands.entity(*scoreboard_entity).with_children(|parent| {
        // Title Header
        parent.spawn((
            Text::new("Scoreboard"),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            Node {
                align_self: AlignSelf::Center,
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
        ));

        for (index, entry) in score_entries.iter().enumerate() {
            // Row Container
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    // Player Name
                    row.spawn((
                        Text::new(format!(
                            "{}) {} #{}",
                            index + 1,
                            entry.player_name,
                            entry.player_id,
                        )),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));

                    // Score Value
                    row.spawn((
                        Text::new(entry.score.to_string()),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.0, 1.0, 0.0)),
                    ));
                });
        }
    });
}

fn render_respawn_popup(
    state: Res<RenderStateResource>,

    mut respawn_popup_visibility: Single<&mut Visibility, With<RespawnPopup>>,
    mut respawn_popup_text: Single<&mut Text, With<RespawnPopupText>>,
) {
    let Some(state) = &state.0 else {
        **respawn_popup_visibility = Visibility::Hidden;
        return;
    };

    let Some(local_player) = &state.local_player else {
        **respawn_popup_visibility = Visibility::Hidden;
        return;
    };

    match local_player.life {
        LifeState::Alive => **respawn_popup_visibility = Visibility::Hidden,
        LifeState::Dead { respawn_time } => {
            **respawn_popup_visibility = Visibility::Visible;
            respawn_popup_text.0 = format!("Respawn in {:.1}", respawn_time);
        }
    }
}

// --- Helper Functions ---

fn create_player(
    commands: &mut Commands,
    player_state: &PlayerState,
    player_marker: PlayerMarker,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    let player_color = match player_marker {
        PlayerMarker::Local => MY_PLAYER_COLOR,
        PlayerMarker::Remote => OTHER_PLAYER_COLOR,
    };

    let health_bar_fg_color = match player_marker {
        PlayerMarker::Local => MY_HEALTH_BAR_FG_COLOR,
        PlayerMarker::Remote => OTHER_HEALTH_BAR_FG_COLOR,
    };

    let mut player_entity = commands.spawn((
        InGameObject,
        Player {
            id: player_state.id,
        },
        Health {
            current: player_state.health,
            max: player_state.max_health,
        },
        Mesh2d(meshes.add(SPACESHIP_SHAPE)),
        MeshMaterial2d(materials.add(player_color)),
        Transform::from_xyz(
            player_state.position.x,
            player_state.position.y,
            PLAYER_LAYER,
        ),
    ));

    match player_marker {
        PlayerMarker::Local => player_entity.insert(LocalPlayer),
        PlayerMarker::Remote => player_entity.insert(RemotePlayer),
    };

    let player_entity_id = player_entity.id();

    player_entity.with_children(|parent| {
        // Name Text
        parent.spawn((
            Text2d::new(format!("{} #{}", &player_state.name, player_state.id)),
            TextLayout::new_with_justify(Justify::Center),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_xyz(0.0, 55.0, PLAYER_INFO_LAYER),
            Billboard {
                offset: Vec2::new(0.0, 55.0),
            },
        ));

        // Health Bar Background
        parent.spawn((
            Mesh2d(meshes.add(Rectangle::from_size(HEALTH_BAR_SIZE))),
            MeshMaterial2d(materials.add(HEALTH_BAR_BG_COLOR)),
            Transform::from_xyz(0.0, 40.0, PLAYER_INFO_LAYER),
            Billboard {
                offset: Vec2::new(0.0, 40.0),
            },
        ));

        // Health Bar Foreground
        parent.spawn((
            HealthBarValue {
                player_entity: player_entity_id,
            },
            Mesh2d(meshes.add(Rectangle::from_size(HEALTH_BAR_SIZE))),
            MeshMaterial2d(materials.add(health_bar_fg_color)),
            Transform::from_xyz(0.0, 40.0, PLAYER_INFO_LAYER + 0.1),
            Billboard {
                offset: Vec2::new(0.0, 40.0),
            },
        ));
    });
}

fn update_player(
    player_state: &PlayerState,
    health: &mut Health,
    transform: &mut Transform,
    visibility: &mut Visibility,
) {
    // Sync health data
    health.current = player_state.health;
    health.max = player_state.max_health;

    // Update rotation
    transform.rotation = Quat::from_rotation_z(player_state.rotation - PI / 2.0);

    // Update translation
    transform.translation.x = player_state.position.x;
    transform.translation.y = player_state.position.y;

    // Update visibility
    *visibility = match player_state.life {
        shared::LifeState::Alive => Visibility::Visible,
        shared::LifeState::Dead { respawn_time: _ } => Visibility::Hidden,
    };
}

fn create_bullet(
    commands: &mut Commands,
    local_player_id: PlayerId,
    bullet_state: &BulletState,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    // Determine color based on owner
    let color = if bullet_state.owner_id == local_player_id {
        MY_BULLET_COLOR
    } else {
        OTHER_BULLET_COLOR
    };

    commands.spawn((
        InGameObject,
        Bullet {
            id: bullet_state.id,
        },
        Mesh2d(meshes.add(BULLET_SHAPE)),
        MeshMaterial2d(materials.add(color)),
        Transform::from_xyz(
            bullet_state.position.x,
            bullet_state.position.y,
            BULLET_LAYER,
        ),
    ));
}

fn update_bullet(bullet_state: &BulletState, transform: &mut Transform) {
    let rotation = Vec2::new(bullet_state.velocity.x, bullet_state.velocity.y).to_angle();
    transform.rotation = Quat::from_rotation_z(rotation - PI / 2.0);

    transform.translation.x = bullet_state.position.x;
    transform.translation.y = bullet_state.position.y;
}
