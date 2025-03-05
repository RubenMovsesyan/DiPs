@group(0) @binding(0)
var start_texture: texture_storage_2d<rgba8unorm, read>;

@group(1) @binding(0)
var temporal_texture_array: binding_array<texture_storage_2d<rgba8unorm, read_write> >;

@group(1) @binding(1)
var<uniform> starting_index: u32;

@group(2) @binding(0)
var output_texture: texture_storage_2d<rgba8unorm, write>;

const SENSITIVITY: f32 = 5.0;
const MEDIAN_ARRAY_SIZE: i32 = 4;

const WINDOW_SIZE: i32 = 3;
const WIN_SIZE_SQUARE = WINDOW_SIZE * WINDOW_SIZE;

// helper funcitons
fn get_intensity(color: vec4<f32>) -> f32 {
    var c_max = max(color.r, color.g);
    c_max = max(c_max, color.b);

    var c_min = min(color.r, color.g);
    c_min = min(c_min, color.b);

    let luminance = (c_max + c_min) / 2.0;

    return luminance;
}

// Maps in input range to a sigmoid function in the output range
// specified
fn sigmoid_map(
    input: f32,
    input_min: f32,
    input_max: f32,
    output_min: f32,
    output_max: f32,
) -> f32 {
    let sig_input = input * ((output_max - output_min) / (input_max - input_min));
    return inv_sigmoid(sig_input);
}

const SIGMOID_HORIZONTAL_SCALAR: f32 = 5.0;

fn sigmoid(
    input: f32,
) -> f32 {
    return 1.0 / (1.0 + exp(-SIGMOID_HORIZONTAL_SCALAR * input)) - 0.5;
}

fn inv_sigmoid(
    input: f32,
) -> f32 {
    return (-log((1.0 / (input + 0.5)) - 1)) / SIGMOID_HORIZONTAL_SCALAR;
}

/// Takes in the coordinates of the pixel and returns the spatial median filter
/// color of that pixel with the set WINDOW_SIZE
fn spatial_median_filter(coords: vec2<u32>, dimensions: vec2<u32>, input_texture: texture_storage_2d<rgba8unorm, read_write>) -> vec4<f32> {
    var median_array: array<f32, WIN_SIZE_SQUARE>;
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
fn compute_main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let dimensions = textureDimensions(output_texture);
    let coords = vec2<u32>(global_id.xy);

    if (coords.x >= dimensions.x || coords.y >= dimensions.y) {
        return;
    }


    var median_array: array<f32, MEDIAN_ARRAY_SIZE>;

    // Apply the spatial filter to the texture that has been changed for future reference
    textureStore(temporal_texture_array[starting_index], coords.xy, spatial_median_filter(coords.xy, dimensions.xy, temporal_texture_array[starting_index]));
   
    // Fill the median array with the values from all the spatially filtered textures
    for (var i = 0; i < MEDIAN_ARRAY_SIZE; i++) {
        median_array[i] = get_intensity(textureLoad(temporal_texture_array[i], coords.xy));
    }

    // Sort the temporl texture array
    for (var i = 0; i < MEDIAN_ARRAY_SIZE; i++) {
        var swapped: bool = false;
        for (var j = 0; j < MEDIAN_ARRAY_SIZE; j++) {
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
    
    let original_intensity = textureLoad(start_texture, coords.xy).r;
    var diff = (original_intensity - median_array[MEDIAN_ARRAY_SIZE / 2]);


    diff = sigmoid_map(diff, -1.0, 1.0, -0.5, 0.5) * SENSITIVITY;
    let new_color = vec3<f32>(0.5, 0.5, 0.5) - vec3<f32>(diff, diff, diff);
    
    textureStore(output_texture, coords.xy, vec4<f32>(new_color.rgb, 1.0));
}
