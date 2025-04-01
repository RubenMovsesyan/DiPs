use std::{collections::HashMap, rc::Rc};

use anyhow::{Result, anyhow};
use dynamic_texture_array::create_dynamic_bindings;
use pollster::FutureExt;
use wgpu::{
    Backends, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, Buffer,
    BufferBindingType, BufferDescriptor, BufferUsages, CommandEncoderDescriptor,
    ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor, Device, DeviceDescriptor,
    Extent3d, Features, Instance, InstanceDescriptor, Limits, Maintain, MapMode, MemoryHints,
    Origin3d, PipelineCompilationOptions, PipelineLayoutDescriptor, PowerPreference, Queue,
    RequestAdapterOptionsBase, ShaderStages, StorageTextureAccess, TexelCopyBufferInfo,
    TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor, TextureViewDimension,
    util::{BufferInitDescriptor, DeviceExt},
};

use crate::utils::indexing::UCircularIndex;

mod dynamic_texture_array;

const WORK_GROUP_WIDTH: u32 = 16;
const WORK_GROUP_HEIGHT: u32 = 16;

// Helper functions
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

#[derive(Debug)]
pub struct DiPsCompute {
    // WGPU
    device: Rc<Device>,
    queue: Rc<Queue>,

    // Compute pipeline for computing the inital texture through
    // the temporal filter
    texture_array_bind_groups: Vec<BindGroup>,
    pre_compute_pipeline: ComputePipeline,

    output_bind_group: BindGroup,

    // input_texture: Texture,
    input_textures: Vec<Texture>,
    output_texture: Texture,
    snapshot_buffer: Buffer,
    output_buffer: Buffer,

    texture_dimensions: Extent3d,

    texture_index: UCircularIndex,
    // The current texture getting run through the pipeline
}

impl DiPsCompute {
    pub fn new(num_textures: usize, textures_width: u32, textures_height: u32) -> Result<Self> {
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&RequestAdapterOptionsBase {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .block_on()
            .ok_or(anyhow!("Couldn't create the adapter"))?;

        let (device, queue) = match adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("Device and Queue"),
                    required_features: Features::TEXTURE_BINDING_ARRAY
                        | Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                    required_limits: Limits {
                        max_bind_groups: 5,
                        ..Default::default()
                    },
                    memory_hints: MemoryHints::default(),
                },
                None,
            )
            .block_on()
        {
            Ok((device, queue)) => (device, queue),
            Err(err) => return Err(anyhow!(err)),
        };

        let (device, queue) = (Rc::new(device), Rc::new(queue));

        let textures = (0..num_textures)
            .map(|i| {
                let texture = device.create_texture(&TextureDescriptor {
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
                &device,
                0,
                textures
                    .iter()
                    .map(|(texture_view, _texture)| texture_view)
                    .collect(),
                "dips_compute/shaders/pre_compute_shader.wgsl",
            );

        let (snapshot_texture_view, snapshot_texture) = {
            let texture = device.create_texture(&TextureDescriptor {
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

            (
                texture.create_view(&TextureViewDescriptor::default()),
                texture,
            )
        };

        let snapshot_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Snapshot buffer"),
            contents: bytemuck::cast_slice(&[0u32]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let (output_texture_view, output_texture, output_buffer) = {
            let texture = device.create_texture(&TextureDescriptor {
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
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
                view_formats: &[],
            });

            let buffer_size = (padded_bytes_per_row(textures_height) as u64
                * textures_height as u64)
                * std::mem::size_of::<u8>() as u64;

            let buffer = device.create_buffer(&BufferDescriptor {
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
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Output Texture Bind Group Layout"),
                entries: &[
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

        let output_bind_group = device.create_bind_group(&BindGroupDescriptor {
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
            device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Pre Compute Pipeline Layout"),
                bind_group_layouts: &texture_array_bind_group_layouts
                    .iter()
                    .chain([&output_bind_group_layout])
                    .map(|layout| layout)
                    .collect::<Vec<&BindGroupLayout>>(),
                push_constant_ranges: &[],
            });

        let pre_compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Pre Compute Pipeline"),
            cache: None,
            layout: Some(&pre_compute_pipeline_layout),
            entry_point: Some("pre_compute_main"),
            module: &modified_shader_module,
            compilation_options: PipelineCompilationOptions {
                constants: &{
                    let mut hm = HashMap::new();
                    hm.insert(String::from("NUM_TEXTURES"), num_textures as f64);
                    hm
                },
                ..Default::default()
            },
        });

        Ok(Self {
            device,
            queue,
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
                height: textures_width,
                depth_or_array_layers: 1,
            },
            texture_index: UCircularIndex::new(0, num_textures),
        })
    }

    pub fn send_frame(&mut self, frame: &[u8], snapshot: Option<()>) -> Vec<u8> {
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Frame Compute Command Encoder"),
            });

        self.queue.write_texture(
            self.input_textures[*self.texture_index.as_ref()].as_image_copy(),
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
            self.queue
                .write_buffer(&self.snapshot_buffer, 0, bytemuck::cast_slice(&[1u32]));
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

        self.queue.submit(Some(encoder.finish()));

        if let Some(_) = snapshot {
            self.queue
                .write_buffer(&self.snapshot_buffer, 0, bytemuck::cast_slice(&[0u32]));
        }

        let out = {
            // Read the buffer from the gpu
            let buffer_slice = self.output_buffer.slice(..);
            buffer_slice.map_async(MapMode::Read, |_| {});
            self.device.poll(Maintain::Wait);

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
