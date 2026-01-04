#import bevy_sprite::mesh2d_vertex_output::VertexOutput
#import bevy_render::globals::Globals

@group(0) @binding(1) var<uniform> globals: Globals;

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> bg_color: vec4<f32>;

fn rand(st: vec2<f32>) -> f32 {
    return fract(sin(dot(st, vec2<f32>(12.9898, 78.233))) * 43758.5453123);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let star_prob = 0.998; 

    var color = 0.0;

    let r = rand(mesh.uv.xy);
    if (r > star_prob) {
        let magnitude = (r - star_prob) / (1.0 - star_prob);

        color = magnitude * (0.85 * sin(globals.time * (magnitude * 5.0) + 720.0 * magnitude) + 0.95);
    }

    return vec4<f32>(vec3<f32>(color), 1.0) + bg_color;
}
