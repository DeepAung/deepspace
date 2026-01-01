#import bevy_sprite::mesh2d_vertex_output::VertexOutput
#import bevy_render::globals::Globals

@group(0) @binding(1) var<uniform> globals: Globals;

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> bg_color: vec4<f32>;

fn rand(st: vec2<f32>) -> f32 {
    return fract(sin(dot(st, vec2<f32>(12.9898, 78.233))) * 43758.5453123);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let size = 50.0;
    let prob = 0.99;

    // mesh.position is equivalent to FRAGCOORD (window pixel coordinates)
    let pos = floor(mesh.position.xy / size);

    var color = 0.0;
    let star_value = rand(pos);

    // 2. Main Star Logic
    if (star_value > prob) {
        let center = size * pos + vec2<f32>(size, size) * 0.5;

        let t = 0.9 + 0.2 * sin(globals.time * 8.0 + (star_value - prob) / (1.0 - prob) * 45.0);

        // Calculate distance
        let dist = distance(mesh.position.xy, center);
        let base_color = 1.0 - dist / (0.5 * size);

        // Calculate the cross flare
        // Added + 0.001 to denominators to prevent division by zero artifacts
        let dist_y = abs(mesh.position.y - center.y);
        let dist_x = abs(mesh.position.x - center.x);

        color = base_color * t / (dist_y + 0.001) * t / (dist_x + 0.001);
    } 
    // 3. Background Twinkle Logic
    else {
        // Emulate SCREEN_UV
        let screen_uv = mesh.uv;

        if (rand(screen_uv.xy / 20.0) > 0.996) {
            let r = rand(screen_uv.xy);
            color = r * (0.85 * sin(globals.time * (r * 5.0) + 720.0 * r) + 0.95);
        }
    }

    // 4. Output
    return vec4<f32>(vec3<f32>(color), 1.0) + bg_color;
}
