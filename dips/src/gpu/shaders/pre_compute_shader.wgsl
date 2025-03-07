@group(0) @binding(0)
var start_texture_array: binding_array<texture_storage_2d<rgba8unorm, read> >;

@group(1) @binding(0)
var output_texture: texture_storage_2d<rgba8unorm, write>;


// Compiled constants
@id(1) override WINDOW_SIZE: i32 = 3;
@id(4) override CHROMA_FILTER: u32 = 0;

override WIN_SIZE_SQUARE: i32 = WINDOW_SIZE * WINDOW_SIZE;

const SENSITIVITY: f32 = 2.0;
const MEDIAN_ARRAY_SIZE: i32 = 4;

const MAX_WIN_SIZE_SQUARE = 11 * 11;

// helper funcitons
fn get_intensity(color: vec4<f32>) -> f32 {
    if (CHROMA_FILTER == 1) {
        return color.r;
    } else if (CHROMA_FILTER == 2) {
        return color.g;
    } else if (CHROMA_FILTER == 3) {
        return color.b;
    }

    var c_max = max(color.r, color.g);
    c_max = max(c_max, color.b);

    var c_min = min(color.r, color.g);
    c_min = min(c_min, color.b);

    let luminance = (c_max + c_min) / 2.0;

    return luminance;
}

/// Takes in the coordinates of the pixel and returns the spatial median filter
/// color of that pixel with the set WINDOW_SIZE
fn spatial_median_filter(coords: vec2<u32>, dimensions: vec2<u32>, input_texture: texture_storage_2d<rgba8unorm, read>) -> vec4<f32> {
    if (WINDOW_SIZE == 1) {
        let intensity = get_intensity(textureLoad(input_texture, coords.xy));
        return vec4<f32>(intensity, intensity, intensity, 1.0);
    }

    
    var median_array: array<f32, MAX_WIN_SIZE_SQUARE>;
    let win_size_2 = WINDOW_SIZE / 2;

    for (var i = -win_size_2; i < win_size_2; i++) {
        for (var j = -win_size_2; j < win_size_2; j++) {
            var color: f32;
            if (i32(coords.x) + i >= i32(dimensions.x) || i32(coords.y) + j >= i32(dimensions.y) || i32(coords.x) + i < 0 || i32(coords.y) + j < 0) {
                color = 0.0;
            } else {
                color = get_intensity(textureLoad(input_texture, vec2<u32>(u32(i32(coords.x) + i), u32(i32(coords.y) + j))));
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
            if (median_array[j] > median_array[j + 1]) {
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

    let intensity = median_array[(WIN_SIZE_SQUARE / 2) + 1];
    return vec4<f32>(intensity, intensity, intensity, 1.0);
}

@compute @workgroup_size(16, 16)
fn pre_compute_main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let dimensions = textureDimensions(output_texture);
    let coords = vec2<u32>(global_id.xy);

    if (coords.x >= dimensions.x || coords.y >= dimensions.y) {
        return;
    }

    // Find the temporal median of the start textures
    var start_median_array: array<f32, MEDIAN_ARRAY_SIZE>;
    start_median_array[0] = get_intensity(spatial_median_filter(coords.xy, dimensions.xy, start_texture_array[0]));
    start_median_array[1] = get_intensity(spatial_median_filter(coords.xy, dimensions.xy, start_texture_array[1]));
    start_median_array[2] = get_intensity(spatial_median_filter(coords.xy, dimensions.xy, start_texture_array[2]));
    start_median_array[3] = get_intensity(spatial_median_filter(coords.xy, dimensions.xy, start_texture_array[3]));

    // Sort the start median array
    for (var i = 0; i < MEDIAN_ARRAY_SIZE; i++) {
        var swapped: bool = false;
        for (var j = 0; j < MEDIAN_ARRAY_SIZE; j++) {
            if (start_median_array[j] > start_median_array[j + 1]) {
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

    let intensity = start_median_array[MEDIAN_ARRAY_SIZE / 2];
    let new_color = vec3<f32>(intensity, intensity, intensity);

    textureStore(output_texture, coords.xy, vec4<f32>(new_color.rgb, 1.0));
}
