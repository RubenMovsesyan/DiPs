
@group(4) @binding(0)
var<uniform> snapshot: u32;

@group(4) @binding(1)
var snapshot_texture: texture_storage_2d<rgba8unorm, read_write>;

@group(4) @binding(2)
var output_texture: texture_storage_2d<rgba8unorm, write>;

override NUM_TEXTURES: u32 = 1;
const MAX_TEMPORAL_ARRAY_SIZE: u32 = 16;


// Compiled Constants
override COLORIZE: bool = true;
override WINDOW_SIZE: i32 = 3;
override SIGMOID_HORIZONTAL_SCALAR: f32 = 5.0;
// 0 = Sigmoid
// 1 = Inverse Sigmoid
override FILTER_TYPE: u32 = 0;
override CHROMA_FILTER: u32 = 0;

override WIN_SIZE_SQUARE = WINDOW_SIZE * WINDOW_SIZE;

const DIFF_SCALE: f32 = 5.0;
const MEDIAN_ARRAY_SIZE: i32 = 4;
const MAX_WIN_SIZE_SQUARE = 11 * 11;

const NEGATIVE_COLOR: f32 = 0.0;
const POSITIVE_COLOR: f32 = 120.0;

// Helper Functions
fn diff_to_color(diff: f32) -> vec3<f32> {
    if (diff < 0) {
        return hsl_to_rgb(NEGATIVE_COLOR, abs(diff), 0.5);
    }

    return hsl_to_rgb(POSITIVE_COLOR, diff, 0.5);
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
        return vec3<f32>(0.0 + m, chroma + m, x + m);
    } else if (h_prime >= 3 && h_prime < 4) {
        return vec3<f32>(0.0 + m, x + m, chroma + m);
    } else if (h_prime >= 4 && h_prime < 5) {
        return vec3<f32>(x + m, 0.0 + m, chroma + m);
    } else if (h_prime >= 5 && h_prime <= 6) {
        return vec3<f32>(chroma + m, 0.0 + m, x + m);
    } else {
        return vec3<f32>(0.0 + m, 0.0 + m, 0.0 + m);
    }
}

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

// Maps in input range to a sigmoid function in the output range
// specified
// fn sigmoid_map(
//     input: f32,
//     input_min: f32,
//     input_max: f32,
//     output_min: f32,
//     output_max: f32,
// ) -> f32 {
//     let sig_input = input * ((output_max - output_min) / (input_max - input_min));
//     return sigmoid(sig_input);
// }

fn map(
    input: f32,
    input_min: f32,
    input_max: f32,
    output_min: f32,
    output_max: f32,
) -> f32 {
    return input * ((output_max - output_min) / (input_max - input_min));
}


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

fn load_from_texture_id(texture_id: u32, coords: vec2<u32>) -> vec4<f32> {
    switch (texture_id) {
//lFtIr3p1Ac3
        default: {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
    }
}

/// Takes in the coordinates of the pixel and returns the spatial median filter
/// color of that pixel with the set WINDOW_SIZE
fn spatial_median_filter(coords: vec2<u32>, dimensions: vec2<u32>, input_texture_id: u32) -> f32 {
    if (WINDOW_SIZE == 1) {
        let intensity = get_intensity(load_from_texture_id(input_texture_id, coords.xy));
        // return vec4<f32>(intensity, intensity, intensity, 1.0);
        return intensity;
    }


    var spatial_median_array: array<f32, MAX_WIN_SIZE_SQUARE>;
    let win_size_2 = WINDOW_SIZE / 2;

    for (var i = -win_size_2; i < win_size_2; i++) {
        for (var j = -win_size_2; j < win_size_2; j++) {
            var color: f32;
            if (i32(coords.x) + i >= i32(dimensions.x) || i32(coords.y) + j >= i32(dimensions.y) || i32(coords.x) + i < 0 || i32(coords.y) + j < 0) {
                color = 0.0;
            } else {
                // color = get_intensity(textureLoad(input_texture, vec2<u32>(u32(i32(coords.x) + i), u32(i32(coords.y) + j))));
                color = get_intensity(load_from_texture_id(input_texture_id, vec2<u32>(u32(i32(coords.x) + i), u32(i32(coords.y) + j))));
            }

            let array_i = i + win_size_2;
            let array_j = j + win_size_2;

            let array_ind = array_i + (WINDOW_SIZE * array_j);

            spatial_median_array[array_ind] = color;
        }
    }

    // sort the array
    for (var i = 0; i < WIN_SIZE_SQUARE - 1; i++) {
        var swapped: bool = false;
        for (var j = 0; j < WIN_SIZE_SQUARE - 1; j++) {
            if (spatial_median_array[j] > spatial_median_array[j + 1]) {
                let temp = spatial_median_array[j];
                spatial_median_array[j] = spatial_median_array[j + 1];
                spatial_median_array[j + 1] = temp;

                swapped = true;
            }
        }

        if (!swapped) {
            break;
        }
    }

    let intensity = spatial_median_array[(WIN_SIZE_SQUARE / 2) + 1];
    // return vec4<f32>(intensity, intensity, intensity, 1.0);
    return intensity;
    // return 0.0;
}

@compute @workgroup_size(16, 16)
fn pre_compute_main(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let dimensions = textureDimensions(output_texture);
    let coords = vec2<u32>(global_id.xy);

    if (coords.x >= dimensions.x || coords.y >= dimensions.y) {
        return;
    }

    // var textures: array<vec4<f32>, MAX_TEMPORAL_ARRAY_SIZE>;
    var median_array: array<f32, MAX_TEMPORAL_ARRAY_SIZE>;
    // =========== array generator ===========
//r3p1Ac3
    // =========== array generator ===========


    // Fill the median array with the values from the spatially filtered textures
    // for (var i: u32 = 0; i < NUM_TEXTURES; i++) {
    //     median_array[i] = get_intensity(textures[i]);
    // }

    // Sort the temporal texture array
    for (var i: u32 = 0; i < NUM_TEXTURES; i++) {
        var swapped: bool = false;
        for (var j: u32 = 0; j < NUM_TEXTURES; j++) {
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



    if (snapshot == 1) {
        let intensity = median_array[NUM_TEXTURES / 2];
        let new_color = vec3<f32>(intensity, intensity, intensity);
        textureStore(snapshot_texture, coords.xy, vec4<f32>(new_color.rgb, 1.0));
        textureStore(output_texture, coords.xy, vec4<f32>(new_color.rgb, 1.0));
    } else {
        let original_intensity = textureLoad(snapshot_texture, coords.xy).r;
        var diff = (original_intensity - median_array[NUM_TEXTURES / 2]);

        diff = map(diff, -1.0, 1.0, -0.5, 0.5);

        switch FILTER_TYPE {
            case 0u: {
                diff = sigmoid(diff);
            }
            case 1u: {
                diff = inv_sigmoid(diff);
            }
            default: {}
        }

        diff *= DIFF_SCALE;
        var new_color: vec3<f32>;

        if (COLORIZE == true) {
            new_color = diff_to_color(diff);
        } else {
            new_color = vec3<f32>(0.5, 0.5, 0.5) - vec3<f32>(diff, diff, diff);
        }

        textureStore(output_texture, coords.xy, vec4<f32>(new_color.rgb, 1.0));
    }
}
