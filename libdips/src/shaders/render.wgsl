@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Create a full-screen triangle
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0)
    );

    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
}

@group(0) @binding(0)
var output_texture: texture_2d<f32>;

@group(0) @binding(1)
var output_sampler: sampler;

@fragment
fn fs_main(
    @builtin(position) pos: vec4<f32>
) -> @location(0) vec4<f32> {
    let texture_dimensions = textureDimensions(output_texture);
    let adj_pos = vec2<f32>(pos.x / f32(texture_dimensions.x), pos.y / f32(texture_dimensions.y));
    let color: vec4<f32> = textureSample(output_texture, output_sampler, adj_pos.xy);

    return color;
}
