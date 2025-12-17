use bevy::prelude::ops::atan;
use bevy::prelude::*;
use shared::{BulletId, PlayerId, TIME_SYNC_DURATION};
use std::collections::HashMap;

use crate::plugins::{GameState, NetworkClient, StreamReceiver};

pub struct GameScreenPlugin;

impl Plugin for GameScreenPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_duration(TIME_SYNC_DURATION))
            .add_systems(OnEnter(GameState::InGame), setup_game_screen)
            .add_systems(OnExit(GameState::InGame), teardown_game_screen)
            .add_systems(
                Update,
                (handle_input, render_game).run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                FixedUpdate,
                time_sync_system.run_if(in_state(GameState::InGame)),
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

const PLAYER_LAYER: f32 = 10.0;
const BULLET_LAYER: f32 = 5.0;

// --- Components ---
#[derive(Component)]
struct GameScreenRoot;

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

// --- Systems ---

fn setup_game_screen(
    mut commands: Commands,
    // mut meshes: ResMut<Assets<Mesh>>,
    // mut materials: ResMut<Assets<ColorMaterial>>,
    // network_client: Res<NetworkClient>,
) {
    info!("GAME STARTED!");
    commands.spawn(GameScreenRoot).insert(Camera2d);

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
