@group(0) @binding(0)
var start_texture: texture_2d<f32>;

@group(0) @binding(1)
var input_texture: texture_2d<f32>;

@group(0) @binding(2)
var output_texture: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn compute_main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let dimensions = textureDimensions(start_texture);
    let coords = vec2<u32>(global_id.xy);

    if (coords.x >= dimensions.x || coords.y >= dimensions.y) {
        return;
    }

    let start_color = textureLoad(start_texture, coords.xy, 0);
    let curr_color = textureLoad(input_texture, coords.xy, 0);

    let diff_color = start_color - curr_color;

    textureStore(output_texture, coords.xy, vec4<f32>(diff_color.rgb, 1.0));

    // let color = textureLoad(input_texture, coords.xy, 0);
    // // let gray = dot(vec3<f32>(0.299, 0.587, 0.114), color.rgb);

    // var c_max = max(color.r, color.g);
    // c_max = max(c_max, color.b);

    // var c_min = min(color.r, color.g);
    // c_min = min(c_min, color.b);

    // let luminance = (c_max + c_min) / 2.0;

    // textureStore(output_texture, coords.xy, vec4<f32>(luminance, luminance, luminance, 1.0));
}
