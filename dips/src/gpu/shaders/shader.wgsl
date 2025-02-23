@group(0) @binding(0)
var start_texture: texture_2d<f32>;

@group(0) @binding(1)
var input_texture: texture_2d<f32>;

@group(0) @binding(2)
var output_texture: texture_storage_2d<rgba8unorm, write>;


// Constants
const WINDOW_SIZE: i32 = 3;
const WIN_SIZE_SQUARE = WINDOW_SIZE * WINDOW_SIZE;

// helper functions
fn get_intensity(color: vec4<f32>) -> f32 {
    var c_max = max(color.r, color.g);
    c_max = max(c_max, color.b);

    var c_min = min(color.r, color.g);
    c_min = min(c_min, color.b);

    let luminance = (c_max + c_min) / 2.0;

    return luminance;
}

@compute @workgroup_size(16, 16)
fn compute_main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let dimensions = textureDimensions(start_texture);
    let coords = vec2<u32>(global_id.xy);

    if (coords.x >= dimensions.x || coords.y >= dimensions.y) {
        return;
    }

    // let start_color = textureLoad(start_texture, coords.xy, 0);
    // let curr_color = textureLoad(input_texture, coords.xy, 0);

    // let diff_color = start_color - curr_color;

    // apply median filter for each pixel
    var median_array: array<vec4<f32>, WIN_SIZE_SQUARE>;
    let win_size_2 = WINDOW_SIZE / 2;
    // Put the pixels and neighboring pixels in an array
    for (var i = -win_size_2; i < win_size_2; i++) {
        for (var j = -win_size_2; j < win_size_2; j++) {
            var color: vec4<f32>;
            if (i32(coords.x) + i >= i32(dimensions.x) || i32(coords.y) + j >= i32(dimensions.y) || i32(coords.x) + i < 0 || i32(coords.y) + j < 0) {
                color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
            } else {
                color = textureLoad(input_texture, vec2<u32>(u32(i32(coords.x) + i), u32(i32(coords.y) + j)), 0);
            }

            let array_i = i + win_size_2;
            let array_j = j + win_size_2;

            let array_ind = array_i + (WINDOW_SIZE * array_j);

            median_array[array_ind] = color;
        }
    }

    // sort the array
    for (var i = 0; i < WIN_SIZE_SQUARE; i++) {
        var swapped: bool = false;
        for (var j = 0; j < WIN_SIZE_SQUARE; j++) {
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

    // textureStore(output_texture, coords.xy, vec4<f32>(diff_color.rgb, 1.0));
    textureStore(output_texture, coords.xy, vec4<f32>(median_array[(WIN_SIZE_SQUARE / 2) + 1].rgb, 1.0));
    // textureStore(output_texture, coords.xy, vec4<f32>(1.0, 1.0, 1.0, 1.0));
}
