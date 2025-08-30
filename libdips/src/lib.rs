use std::sync::Arc;

use anyhow::Result;
use circular_index::UCircularIndex;
use gpu_controller::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBindingType,
    BufferDescriptor, BufferInitDescriptor, BufferUsages, Color, ComputePassDescriptor,
    ComputePipeline, ComputePipelineDescriptor, Extent3d, GpuController, LoadOp, MaintainBase,
    MapMode, Operations, Origin3d, PipelineCompilationOptions, PipelineLayoutDescriptor,
    RenderPassColorAttachment, RenderPassDescriptor, ShaderModule, ShaderStages,
    StorageTextureAccess, StoreOp, SurfaceTexture, TexelCopyBufferInfo, TexelCopyBufferLayout,
    TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
};
pub use properties::{ChromaFilter, DiPsProperties, Filter};
use renderer::Renderer;

mod circular_index;
mod properties;
mod renderer;
mod window;

const WORK_GROUP_WIDTH: u32 = 16;
const WORK_GROUP_HEIGHT: u32 = 16;

fn compute_work_group_count(
    (width, height): (u32, u32),
    (work_group_width, work_group_height): (u32, u32),
) -> (u32, u32) {
    let x = (width + work_group_width - 1) / work_group_width;
    let y = (height + work_group_height - 1) / work_group_height;

    (x, y)
}

fn padded_bytes_per_row(width: u32) -> usize {
    let bytes_per_row = width as usize * 4;
    let padding = (256 - bytes_per_row % 256) % 256;
    bytes_per_row + padding
}

// ============= Dynamic Texture Array =============
fn create_dynamic_bindings(
    gpu_controller: &GpuController,
    mut bind_group: u32,
    texture_views: Vec<&TextureView>,
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

            let bind_group_layout =
                gpu_controller.create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: Some("Texture Array Bind Group Layout"),
                    entries: &layout_entries,
                });

            let bind_group = gpu_controller.create_bind_group(&BindGroupDescriptor {
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
        let bind_group_layout =
            gpu_controller.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Texture Array Bind Group Layout"),
                entries: &layout_entries,
            });

        let bind_group = gpu_controller.create_bind_group(&BindGroupDescriptor {
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
        let l = gpu_controller.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Dummy Bind Group Layout"),
            entries: &[],
        });

        bind_groups.push(gpu_controller.create_bind_group(&BindGroupDescriptor {
            label: Some("Dummy Bind Group"),
            layout: &l,
            entries: &[],
        }));

        layouts.push(l);
    }

    let mut modified_shader = String::new();

    let mut shader = include_str!("shaders/pre_compute_shader.wgsl").to_string();
    shader = shader.replace("//r3p1Ac3", &arraying_texture);
    shader = shader.replace("//lFtIr3p1Ac3", &texture_loading);

    modified_shader.push_str(&shader_bindings);
    modified_shader.push_str(&shader);

    // println!("{modified_shader}");
    // println!("{:#?}", layouts);

    let shader_module = gpu_controller.create_shader(&modified_shader);

    (layouts, bind_groups, shader_module)
}

pub struct DiPs {
    gpu_controller: Arc<GpuController>,

    texture_array_bind_groups: Vec<BindGroup>,
    pre_compute_pipeline: ComputePipeline,

    output_bind_group: BindGroup,

    input_textures: Vec<Texture>,
    output_texture: Texture,
    snapshot_buffer: Buffer,
    output_buffer: Buffer,

    texture_dimensions: Extent3d,

    texture_index: UCircularIndex,
}

impl DiPs {
    pub fn new(
        num_textures: usize,
        textures_width: u32,
        textures_height: u32,
        gpu_controller: Arc<GpuController>,
        dips_properties: DiPsProperties,
    ) -> Result<Self> {
        let textures = (0..num_textures)
            .into_iter()
            .map(|i| {
                let texture = gpu_controller.create_texture(&TextureDescriptor {
                    label: Some(&format!("Texture: {}", i)),
                    size: Extent3d {
                        width: textures_height,
                        height: textures_width,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba8Unorm,
                    usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                (
                    texture.create_view(&TextureViewDescriptor::default()),
                    texture,
                )
            })
            .collect::<Vec<_>>();

        let (texture_array_bind_group_layouts, texture_array_bind_groups, modified_shader_module) =
            create_dynamic_bindings(
                &gpu_controller,
                0,
                textures
                    .iter()
                    .map(|(texture_view, _texture)| texture_view)
                    .collect(),
            );

        let snapshot_texture_view = {
            let texture = gpu_controller.create_texture(&TextureDescriptor {
                label: Some("Snapshot texture"),
                size: Extent3d {
                    width: textures_height,
                    height: textures_width,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });

            texture.create_view(&TextureViewDescriptor::default())
        };

        let snapshot_buffer = gpu_controller.create_buffer_init(&BufferInitDescriptor {
            label: Some("Snapshot buffer"),
            contents: bytemuck::cast_slice(&[0u32]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let (output_texture_view, output_texture, output_buffer) = {
            let texture = gpu_controller.create_texture(&TextureDescriptor {
                label: Some("Output texture"),
                size: Extent3d {
                    width: textures_height,
                    height: textures_width,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::STORAGE_BINDING
                    | TextureUsages::COPY_SRC
                    | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let buffer_size = (padded_bytes_per_row(textures_height) as u64
                * textures_height as u64)
                * std::mem::size_of::<u8>() as u64;

            let buffer = gpu_controller.create_buffer(&BufferDescriptor {
                label: Some("Output Texture Buffer"),
                size: buffer_size,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            (
                texture.create_view(&TextureViewDescriptor::default()),
                texture,
                buffer,
            )
        };

        let output_bind_group_layout =
            gpu_controller.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Output Texture Bind Group Layout"),
                entries: &[
                    // Snapshot determiner binding
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Snapshot texture
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: StorageTextureAccess::ReadWrite,
                            format: TextureFormat::Rgba8Unorm,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    // Output Texture
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: StorageTextureAccess::WriteOnly,
                            format: TextureFormat::Rgba8Unorm,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let output_bind_group = gpu_controller.create_bind_group(&BindGroupDescriptor {
            label: Some("Output Texture Bind Group"),
            layout: &output_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: snapshot_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&snapshot_texture_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&output_texture_view),
                },
            ],
        });

        let pre_compute_pipeline_layout =
            gpu_controller.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Pre Compute Pipeline Layout"),
                bind_group_layouts: &texture_array_bind_group_layouts
                    .iter()
                    .chain([&output_bind_group_layout])
                    .map(|layout| layout)
                    .collect::<Vec<&BindGroupLayout>>(),
                push_constant_ranges: &[],
            });

        let pre_compute_pipeline =
            gpu_controller.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Pre Compute Pipeline"),
                cache: None,
                layout: Some(&pre_compute_pipeline_layout),
                entry_point: Some("pre_compute_main"),
                module: &modified_shader_module,
                compilation_options: PipelineCompilationOptions {
                    constants: &dips_properties.get_properties_slice(),
                    ..Default::default()
                },
            });

        Ok(Self {
            gpu_controller,
            texture_array_bind_groups,
            pre_compute_pipeline,
            output_bind_group,
            input_textures: textures
                .into_iter()
                .map(|(_texture_view, texture)| texture)
                .collect::<Vec<Texture>>(),
            snapshot_buffer,
            output_texture,
            output_buffer,
            texture_dimensions: Extent3d {
                width: textures_height,
                height: textures_height,
                depth_or_array_layers: 1,
            },
            texture_index: UCircularIndex::new(0, num_textures),
        })
    }

    pub fn send_frame(
        &mut self,
        frame: &[u8],
        snapshot: Option<()>,
        surface_texture: Option<&SurfaceTexture>,
    ) -> Vec<u8> {
        let mut encoder = self
            .gpu_controller
            .create_command_encoder("Frame Compute Command Encoder");

        self.gpu_controller.write_texture(
            &self.input_textures[*self.texture_index.as_ref()],
            frame,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(
                    self.texture_dimensions.width * std::mem::size_of::<f32>() as u32,
                ),
                rows_per_image: Some(self.texture_dimensions.height),
            },
            self.texture_dimensions,
        );

        self.texture_index += 1;

        if let Some(_) = snapshot {
            self.gpu_controller.write_buffer(
                &self.snapshot_buffer,
                0,
                bytemuck::cast_slice(&[1u32]),
            );
        }

        {
            let (dispatch_width, dispatch_height) = compute_work_group_count(
                (
                    self.texture_dimensions.width,
                    self.texture_dimensions.height,
                ),
                (WORK_GROUP_WIDTH, WORK_GROUP_HEIGHT),
            );

            // Begin the compute pass
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Frame Compute Pass"),
                timestamp_writes: None,
            });

            // Set the pipeline
            compute_pass.set_pipeline(&self.pre_compute_pipeline);

            // Set the bind groups
            // compute_pass.set_bind_group(0, &self.texture_array_bind_group, &[]);
            for (index, bind_group) in self.texture_array_bind_groups.iter().enumerate() {
                compute_pass.set_bind_group(index as u32, bind_group, &[]);
            }
            compute_pass.set_bind_group(4, &self.output_bind_group, &[]);

            // Dispatch the work groups
            compute_pass.dispatch_workgroups(dispatch_width, dispatch_height, 1);
        }

        let padded_bytes_per_row = padded_bytes_per_row(self.texture_dimensions.width);
        let unpadded_bytes_per_row =
            self.texture_dimensions.width * std::mem::size_of::<f32>() as u32;

        // If we have a renderer attached then render to the screen
        // otherwise just copy to the output buffer
        // if let Some(renderer) = self.renderer.as_ref() {
        //     let output = surface_texture.unwrap();
        //     let view = output
        //         .texture
        //         .create_view(&TextureViewDescriptor::default());

        //     {
        //         let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
        //             label: Some("DiPs Render Pass"),
        //             color_attachments: &[Some(RenderPassColorAttachment {
        //                 view: &view,
        //                 resolve_target: None,
        //                 ops: Operations {
        //                     load: LoadOp::Clear(Color::BLUE),
        //                     store: StoreOp::Store,
        //                 },
        //             })],
        //             ..Default::default()
        //         });

        //         // Set the rendering pipeline
        //         render_pass.set_pipeline(&renderer.pipeline);

        //         // Set the bind group
        //         render_pass.set_bind_group(0, &renderer.bind_group, &[]);

        //         // Render to the screen
        //         render_pass.draw(0..6, 0..1);
        //     }
        //     self.gpu_controller.submit(encoder);
        //     // output.present();
        // } else {
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                aspect: TextureAspect::All,
                texture: &self.output_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
            },
            TexelCopyBufferInfo {
                buffer: &self.output_buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row as u32),
                    rows_per_image: Some(self.texture_dimensions.height as u32),
                },
            },
            self.texture_dimensions,
        );
        self.gpu_controller.submit(encoder);
        // }

        if let Some(_) = snapshot {
            self.gpu_controller.write_buffer(
                &self.snapshot_buffer,
                0,
                bytemuck::cast_slice(&[0u32]),
            );
        }

        let out = {
            // Read the buffer from the gpu
            let buffer_slice = self.output_buffer.slice(..);
            buffer_slice.map_async(MapMode::Read, |_| {});
            _ = self.gpu_controller.poll(MaintainBase::Wait);

            let padded_data = buffer_slice.get_mapped_range();
            let mut output_texture =
                vec![0u8; (unpadded_bytes_per_row * self.texture_dimensions.height) as usize];

            for (padded, pixels) in padded_data
                .chunks_exact(padded_bytes_per_row)
                .zip(output_texture.chunks_exact_mut(unpadded_bytes_per_row as usize))
            {
                pixels.copy_from_slice(&padded[..unpadded_bytes_per_row as usize]);
            }

            drop(padded_data);
            self.output_buffer.unmap();

            output_texture
        };

        out
    }
}
