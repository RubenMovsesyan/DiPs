use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Device, ShaderModule,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StorageTextureAccess, TextureFormat,
    TextureView, TextureViewDimension,
};

use std::{borrow::Cow, fs::read_to_string};

fn load_shader(shader_path: &str) -> String {
    read_to_string(shader_path).expect("Failed to open file")
}

pub fn create_dynamic_bindings(
    device: &Device,
    mut bind_group: u32,
    texture_views: Vec<&TextureView>,
    shader_path: &str,
) -> (Vec<BindGroupLayout>, Vec<BindGroup>, ShaderModule) {
    let mut layouts: Vec<BindGroupLayout> = Vec::new();
    let mut bind_groups: Vec<BindGroup> = Vec::new();

    let mut layout_entries: Vec<BindGroupLayoutEntry> = Vec::new();
    let mut bind_group_entries: Vec<BindGroupEntry> = Vec::new();

    let mut shader_bindings: String = String::new();
    let mut arraying_texture: String = String::new();
    let mut texture_loading: String = String::new();

    for (index, texture_view) in texture_views.iter().enumerate() {
        if index % 4 == 0 && index != 0 {
            bind_group += 1;

            let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Texture Array Bind Group Layout"),
                entries: &layout_entries,
            });

            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Texture Array Bind Group"),
                layout: &bind_group_layout,
                entries: &bind_group_entries,
            });

            layouts.push(bind_group_layout);
            bind_groups.push(bind_group);

            layout_entries.clear();
            bind_group_entries.clear();
        }

        let binding_number = index % 4;
        layout_entries.push(BindGroupLayoutEntry {
            binding: binding_number as u32,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::ReadOnly,
                format: TextureFormat::Rgba8Unorm,
                view_dimension: TextureViewDimension::D2,
            },
            count: None,
        });

        bind_group_entries.push(BindGroupEntry {
            binding: binding_number as u32,
            resource: BindingResource::TextureView(texture_view),
        });

        shader_bindings.push_str(
            &format!("@group({bind_group}) @binding({binding_number})\nvar texture_{index}: texture_storage_2d<rgba8unorm, read>;\n")
        );
        arraying_texture.push_str(&format!(
            "    median_array[{index}] = spatial_median_filter(coords.xy, dimensions.xy, {index});\n" // "    textures[{index}] = textureLoad(texture_{index}, coords.xy);\n"
        ));
        texture_loading.push_str(&format!(
            "        case {index}u: {{\n            return textureLoad(texture_{index}, coords.xy);\n        }}\n"
        ));
    }

    if !layout_entries.is_empty() {
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Texture Array Bind Group Layout"),
            entries: &layout_entries,
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Texture Array Bind Group"),
            layout: &bind_group_layout,
            entries: &bind_group_entries,
        });

        layouts.push(bind_group_layout);
        bind_groups.push(bind_group);

        layout_entries.clear();
        bind_group_entries.clear();
    }

    // Create dummy bind groups to fill in the gap to the required bind groups
    while layouts.len() < 4 {
        let l = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Dummy Bind Group Layout"),
            entries: &[],
        });

        bind_groups.push(device.create_bind_group(&BindGroupDescriptor {
            label: Some("Dummy Bind Group"),
            layout: &l,
            entries: &[],
        }));

        layouts.push(l);
    }

    let mut modified_shader = String::new();

    let mut shader = load_shader(shader_path);
    shader = shader.replace("//r3p1Ac3", &arraying_texture);
    shader = shader.replace("//lFtIr3p1Ac3", &texture_loading);

    modified_shader.push_str(&shader_bindings);
    modified_shader.push_str(&shader);

    // println!("{modified_shader}");
    // println!("{:#?}", layouts);

    let shader_module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Modified Shader Module"),
        source: ShaderSource::Wgsl(Cow::from(modified_shader)),
    });

    (layouts, bind_groups, shader_module)
}
