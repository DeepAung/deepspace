use bevy::ecs::query::QuerySingleError;
use bevy::math::ops::atan2;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{Material2d, Material2dPlugin};
use bevy::time::common_conditions::on_timer;
use shared::{BulletId, PlayerId, TICK_DURATION, WORLD_HEIGHT, WORLD_WIDTH};
use std::collections::HashMap;

use crate::plugins::{GameState, network::NetworkClient};

pub struct GameScreenPlugin;

impl Plugin for GameScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<GridMaterial>::default())
            .add_systems(OnEnter(GameState::InGame), setup_game_screen)
            .add_systems(OnExit(GameState::InGame), teardown_game_screen)
            .add_systems(
                Update,
                (
                    handle_input.run_if(on_timer(TICK_DURATION)),
                    update_camera,
                    render_game,
                )
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

const BULLET_SHAPE: Rectangle = Rectangle::new(2.0, 5.0);

const MY_PLAYER_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);
const OTHER_PLAYER_COLOR: Color = Color::srgb(1.0, 0.0, 0.0);
const MY_BULLET_COLOR: Color = Color::srgb(0.0, 0.0, 1.0);
const OTHER_BULLET_COLOR: Color = Color::srgb(1.0, 0.0, 0.0);

const BACKGROUND_LAYER: f32 = 0.0;
const BULLET_LAYER: f32 = 5.0;
const PLAYER_LAYER: f32 = 10.0;

// --- Components ---

#[derive(Component)]
struct InGameObject;

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
    // network_client: Res<NetworkClient>,
) {
    info!("GAME STARTED!");

    commands.spawn((
        InGameObject,
        Mesh2d(meshes.add(Rectangle::new(WORLD_WIDTH, WORLD_HEIGHT))),
        MeshMaterial2d(grid_materials.add(background_material())),
        Transform::from_xyz(0.0, 0.0, BACKGROUND_LAYER),
    ));
}

fn background_material() -> GridMaterial {
    const LINE_COLOR: LinearRgba = LinearRgba::new(0.03, 0.03, 0.03, 1.0);
    const BG_COLOR: LinearRgba = LinearRgba::new(0.0, 0.0, 0.0, 1.0);
    const GRID_SIZE: f32 = 100.0;
    const THICKNESS: f32 = 0.05;

    GridMaterial {
        color: LINE_COLOR,
        bg_color: BG_COLOR,
        grid_size: GRID_SIZE,
        thickness: THICKNESS,
    }
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

    // Applies a smooth effect to camera movement using stable interpolation
    // between the camera position and the player position on the x and y axes.
    camera
        .translation
        .smooth_nudge(&direction, CAMERA_DECAY_RATE, time.delta_secs());
}

fn handle_input(
    kb_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut network_client: ResMut<NetworkClient>,
) {
    let mut direction = shared::Vec2::new(0.0, 0.0);

    if kb_input.pressed(KeyCode::KeyW) {
        direction.y += 1.;
    }

    if kb_input.pressed(KeyCode::KeyS) {
        direction.y -= 1.;
    }

    if kb_input.pressed(KeyCode::KeyA) {
        direction.x -= 1.;
    }

    if kb_input.pressed(KeyCode::KeyD) {
        direction.x += 1.;
    }

    let shoot = mouse_input.just_pressed(MouseButton::Left);

    println!("Gonna send input");
    // network_client.send_input(direction, shoot).unwrap();
}

fn render_game(
    mut commands: Commands,
    network_client: Res<NetworkClient>,

    mut camera: Single<&mut Transform, With<Camera2d>>,

    mut local_player_query: Query<(Entity, &mut Transform), (With<LocalPlayer>, Without<Camera2d>)>,
    mut remote_players_query: Query<
        (Entity, &Player, &mut Transform),
        (With<RemotePlayer>, Without<Camera2d>, Without<LocalPlayer>),
    >,
    mut bullets_query: Query<
        (Entity, &Bullet, &mut Transform),
        (
            With<Bullet>,
            Without<Camera2d>,
            Without<LocalPlayer>,
            Without<RemotePlayer>,
        ),
    >,

    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let state = network_client.get_render_state();

    println!("get render state {:?}", state);

    // --- LOCAL PLAYER ---
    let Some(local_player_state) = state.local_player else {
        debug!("no local_player");
        return;
    };

    match local_player_query.single_mut() {
        Ok((_, mut transform)) => {
            let angle = {
                let vec = local_player_state.velocity;
                atan2(vec.y, vec.x)
            };
            transform.rotation = Quat::from_rotation_z(angle);

            let pos = local_player_state.position;
            transform.translation.x = pos.x;
            transform.translation.y = pos.y;
        }
        Err(QuerySingleError::NoEntities(_)) => {
            let pos = Vec2::new(local_player_state.position.x, local_player_state.position.y);
            commands.spawn((
                InGameObject,
                Player {
                    id: local_player_state.id,
                },
                LocalPlayer,
                Mesh2d(meshes.add(SPACESHIP_SHAPE)),
                MeshMaterial2d(materials.add(MY_PLAYER_COLOR)),
                Transform::from_xyz(pos.x, pos.y, PLAYER_LAYER),
            ));

            camera.translation.x = pos.x;
            camera.translation.y = pos.y;
        }
        Err(QuerySingleError::MultipleEntities(_)) => {
            panic!("There are multiple instances of LocalPlayer");
        }
    };

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
        commands.spawn((
            InGameObject,
            Player { id },
            RemotePlayer,
            Mesh2d(meshes.add(SPACESHIP_SHAPE)),
            MeshMaterial2d(materials.add(OTHER_PLAYER_COLOR)),
            Transform::from_xyz(pos.x, pos.y, PLAYER_LAYER),
        ));
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

        commands.spawn((
            InGameObject,
            Bullet { id },
            Mesh2d(meshes.add(BULLET_SHAPE)),
            MeshMaterial2d(materials.add(color)),
            Transform::from_xyz(pos.x, pos.y, BULLET_LAYER),
        ));
    }
}
