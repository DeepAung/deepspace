mod game_screen;
mod network;
mod welcome_screen;

use bevy::camera::{ScalingMode, Viewport};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested, WindowResolution};

use crate::plugins::game_screen::GameScreenPlugin;
use crate::plugins::network::{NetworkClient, NetworkPlugin};
use crate::plugins::welcome_screen::WelcomeScreenPlugin;

// --- States ---
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum GameState {
    #[default]
    Welcome,
    InGame,
}

const TARGET_WIDTH: f32 = 1920.0;
const TARGET_HEIGHT: f32 = 1080.0;

pub fn init_game() -> anyhow::Result<()> {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            close_when_requested: false,
            primary_window: Some(Window {
                resolution: WindowResolution::new(TARGET_WIDTH as u32, TARGET_HEIGHT as u32),
                title: "Some Really Cool Title".to_string(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<GameState>()
        .add_plugins(NetworkPlugin)
        .add_plugins(WelcomeScreenPlugin)
        .add_plugins(GameScreenPlugin)
        .add_systems(Startup, setup_camera)
        .add_systems(Update, (update_camera_viewport, disconnect_on_close_window))
        .run();

    Ok(())
}

fn setup_camera(mut commands: Commands) {
    let mut projection = OrthographicProjection::default_2d();
    projection.scaling_mode = ScalingMode::FixedVertical {
        viewport_height: TARGET_HEIGHT,
    };

    commands.spawn((Camera2d, Projection::Orthographic(projection)));
}

fn update_camera_viewport(
    window: Single<&Window, With<PrimaryWindow>>,
    mut camera: Single<&mut Camera>,
    mut ui_scale: ResMut<UiScale>,
) {
    let win_w = window.physical_width();
    let win_h = window.physical_height();

    // 1. Calculate the "Safe Zone" (The size the game should be)
    let target_ratio = TARGET_WIDTH / TARGET_HEIGHT;
    let window_ratio = win_w as f32 / win_h as f32;

    let viewport_w: u32;
    let viewport_h: u32;

    if window_ratio > target_ratio {
        // Window is too wide (Black bars on left/right)
        // Fix the height to the window height, calculate width based on ratio
        viewport_h = win_h;
        viewport_w = (win_h as f32 * target_ratio) as u32;
    } else {
        // Window is too tall (Black bars on top/bottom)
        // Fix the width to the window width, calculate height based on ratio
        viewport_w = win_w;
        viewport_h = (win_w as f32 / target_ratio) as u32;
    }

    // 2. Calculate position to center the viewport (margin: auto)
    // We use integer division here which is fine for pixels
    let x = (win_w - viewport_w) / 2;
    let y = (win_h - viewport_h) / 2;

    // 3. Apply the Viewport to the camera
    camera.viewport = Some(Viewport {
        physical_position: UVec2::new(x, y),
        physical_size: UVec2::new(viewport_w, viewport_h),
        ..default()
    });

    // 4. Fix UI Scaling
    // Since the camera view is now smaller than the window, we scale UI
    // based on the VIEWPORT height, not the window height.
    ui_scale.0 = viewport_h as f32 / TARGET_HEIGHT;
}

fn disconnect_on_close_window(
    mut commands: Commands,
    mut events: MessageReader<WindowCloseRequested>,
    mut network_client: ResMut<NetworkClient>,
) {
    for event in events.read() {
        info!("graceful shutdown: disconnect the player");
        if network_client.connected() {
            if let Err(e) = network_client.disconnect() {
                error!("Failed to disconnect: {:?}", e);
            }
        }

        commands.entity(event.window).despawn();
    }
}
