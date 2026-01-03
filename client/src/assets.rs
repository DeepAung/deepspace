use bevy::{
    prelude::*, render::render_resource::AsBindGroup, shader::ShaderRef, sprite_render::Material2d,
};

// --- Assets Path ---

pub const FONT_PATH: &'static str = "fonts/Zen_Dots/ZenDots-Regular.ttf";

// --- Materials ---

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GridBackgroundMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub bg_color: LinearRgba,
    #[uniform(2)]
    pub grid_size: f32, // How many grid cells across the image
    #[uniform(3)]
    pub thickness: f32, // Thickness of the lines (0.0 to 1.0 relative to cell size)
}

impl Material2d for GridBackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/grid_background.wgsl".into()
    }
}

impl Default for GridBackgroundMaterial {
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

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SpaceBackgroundMaterial {
    #[uniform(0)]
    pub bg_color: LinearRgba,
}

impl Material2d for SpaceBackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/space_background.wgsl".into()
    }
}

impl Default for SpaceBackgroundMaterial {
    fn default() -> Self {
        const BG_COLOR: LinearRgba = LinearRgba::new(0.0, 0.0, 0.0, 1.0);

        Self { bg_color: BG_COLOR }
    }
}
