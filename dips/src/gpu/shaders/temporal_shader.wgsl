@group(0) @binding(0)
var start_texture: texture_storage_2d<rgba8unorm, read>;

// @group(0) @binding(1)
// var temporal_textures: array<texture_2d<f32>>;
@group(0) @binding(1)
var temporal_texture_1: texture_storage_2d<rgba8unorm, read>;

@group(0) @binding(2)
var temporal_texture_2: texture_storage_2d<rgba8unorm, read>;

@group(1) @binding(0)
var temporal_texture_3: texture_storage_2d<rgba8unorm, read>;

@group(1) @binding(1)
var temporal_texture_4: texture_storage_2d<rgba8unorm, read>;

@group(1) @binding(2)
var temporal_texture_5: texture_storage_2d<rgba8unorm, read>;

@group(1) @binding(3)
var output_texture: texture_storage_2d<rgba8unorm, write>;


// const L = arrayLength(temporal_textures);

// helper funcitons
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

    // textureStore(output_texture, coords.xy, vec4<f32>(1.0, 1.0, 1.0, 1.0));

    // let l = arrayLength(temporal_textures);
    // Apply Median filter temporally
    var median_array: array<vec4<f32>, 5>;

    // for (var i = 0; i < 5; i++) {
    //     median_array[i] = textureLoad(temporal_textures[i], coords.xy, 0);
    // }
    median_array[0] = textureLoad(temporal_texture_1, coords.xy);
    median_array[1] = textureLoad(temporal_texture_2, coords.xy);
    median_array[2] = textureLoad(temporal_texture_3, coords.xy);
    median_array[3] = textureLoad(temporal_texture_4, coords.xy);
    median_array[4] = textureLoad(temporal_texture_5, coords.xy);


    // sort the array
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

    textureStore(output_texture, coords.xy, vec4<f32>(median_array[2].rgb, 1.0));
}
