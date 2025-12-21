#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> bg_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> grid_size: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> thickness: f32;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let scaled_uv = mesh.uv * grid_size;

    // 2. Anti-aliasing Logic using fwidth (screen-space derivatives)
    // fract(scaled_uv) - 0.5 centers the coordinate from -0.5 to 0.5
    let uv_centered = fract(scaled_uv) - 0.5;

    // Calculate distance to the nearest edge (0 at edge, 0.5 at center)
    let dist_to_edge = 0.5 - abs(uv_centered);

    // fwidth calculates how much the UV changes per screen pixel.
    // We use this to soften the edge over the span of 1-2 pixels.
    let delta = fwidth(scaled_uv);

    // Smoothstep creates the fade. 
    // We define the line width relative to the screen pixel size (delta) if we want constant width,
    // OR relative to UV space if we use thickness directly.
    // This implementation uses your UV-based thickness but smooths the edges.
    let half_thick = thickness * 0.5;

    let line_x = smoothstep(half_thick - delta.x, half_thick + delta.x, dist_to_edge.x);
    let line_y = smoothstep(half_thick - delta.y, half_thick + delta.y, dist_to_edge.y);

    // Invert because we calculated distance from edge (where 0 is edge)
    // We want 1.0 where the line is.
    let line_strength = 1.0 - min(line_x, line_y);

    return mix(bg_color, color, line_strength);
}
