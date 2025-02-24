@group(0) @binding(0)
var start_texture_array: binding_array<texture_storage_2d<rgba8unorm, read> >;

@group(1) @binding(0)
var temporal_texture_array: binding_array<texture_storage_2d<rgba8unorm, read> >;

@group(2) @binding(0)
var output_texture: texture_storage_2d<rgba8unorm, write>;

const SENSITIVITY: f32 = 360.0;

// helper funcitons
fn get_intensity(color: vec4<f32>) -> f32 {
    var c_max = max(color.r, color.g);
    c_max = max(c_max, color.b);

    var c_min = min(color.r, color.g);
    c_min = min(c_min, color.b);

    let luminance = (c_max + c_min) / 2.0;

    return luminance;
}

// h must be between 0 and 360
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> vec3<f32> {
    let chroma = s * (1 - abs(2 * l - 1));
    let h_prime = h / 60.0;
    let x = chroma * (1 - abs(h_prime % 2.0 - 1));

    let m = l - chroma / 2.0;

    if (h_prime >= 0 && h_prime < 1) {
        return vec3<f32>(chroma + m, x + m, 0.0 + m);
    } else if (h_prime >= 1 && h_prime < 2) {
        return vec3<f32>(x + m, chroma + m, 0.0 + m);
    } else if (h_prime >= 2 && h_prime < 3) {
        return vec3<f32>(x + m, chroma + m, 0.0 + m);
    } else if (h_prime >= 3 && h_prime < 4) {
        return vec3<f32>(x + m, chroma + m, 0.0 + m);
    } else if (h_prime >= 4 && h_prime < 5) {
        return vec3<f32>(x + m, chroma + m, 0.0 + m);
    } else if (h_prime >= 5 && h_prime < 6) {
        return vec3<f32>(x + m, chroma + m, 0.0 + m);
    } else {
        return vec3<f32>(0.0 + m, 0.0 + m, 0.0 + m);
    }
}

@compute @workgroup_size(16, 16)
fn compute_main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let dimensions = textureDimensions(output_texture);
    let coords = vec2<u32>(global_id.xy);

    if (coords.x >= dimensions.x || coords.y >= dimensions.y) {
        return;
    }


    // Find the temporal median of the start textures
    var start_median_array: array<vec4<f32>, 4>;
    start_median_array[0] = textureLoad(start_texture_array[0], coords.xy);
    start_median_array[1] = textureLoad(start_texture_array[1], coords.xy);
    start_median_array[2] = textureLoad(start_texture_array[2], coords.xy);
    start_median_array[3] = textureLoad(start_texture_array[3], coords.xy);

    // Sort the start median array
    for (var i = 0; i < 5; i++) {
        var swapped: bool = false;
        for (var j = 0; j < 5; j++) {
            if (get_intensity(start_median_array[j]) > get_intensity(start_median_array[j + 1])) {
                let temp = start_median_array[j];
                start_median_array[j] = start_median_array[j + 1];
                start_median_array[j + 1] = temp;

                swapped = true;
            }
        }

        if (!swapped) {
            break;
        }
    }


    
    var median_array: array<vec4<f32>, 4>;
    median_array[0] = textureLoad(temporal_texture_array[0], coords.xy);
    median_array[1] = textureLoad(temporal_texture_array[1], coords.xy);
    median_array[2] = textureLoad(temporal_texture_array[2], coords.xy);
    median_array[3] = textureLoad(temporal_texture_array[3], coords.xy);

    // Sort the temporl texture array
    for (var i = 0; i < 5; i++) {
        var swapped: bool = false;
        for (var j = 0; j < 5; j++) {
            if (get_intensity(median_array[j]) > get_intensity(median_array[j + 1])) {
                let temp = median_array[j];
                median_array[j] = median_array[j + 1];
                median_array[j + 1] = temp;

                swapped = true;
            }
        }

        if (!swapped) {
            break;
        }
    }
    
    let original_intensity = get_intensity(start_median_array[2]);
    let diff = (original_intensity - get_intensity(median_array[2])) * SENSITIVITY;

    let new_color = hsl_to_rgb(diff, 1.0, 0.5);
    
    textureStore(output_texture, coords.xy, vec4<f32>(new_color.rgb, 1.0));
}
