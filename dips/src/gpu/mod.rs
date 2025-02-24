use std::collections::VecDeque;

use log::*;
use pollster::*;
use wgpu::{
    include_wgsl, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, Buffer,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor,
    ComputePipeline, ComputePipelineDescriptor, Device, DeviceDescriptor, Extent3d, Features,
    Instance, InstanceDescriptor, Limits, Maintain, MapMode, MemoryHints, Origin3d,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PowerPreference, Queue,
    RequestAdapterOptionsBase, ShaderStages, StorageTextureAccess, TexelCopyBufferInfo,
    TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};

const TEMPORAL_BUFFER_SIZE: usize = 5;

pub struct ComputeState {
    device: Device,
    queue: Queue,

    compute_pipeline: ComputePipeline,
    texture_bind_group: Option<BindGroup>,
    texture_bind_group_2: Option<BindGroup>,
    texture_bind_group_layout: BindGroupLayout,
    texture_bind_group_layout_2: BindGroupLayout,
    texture_dimensions: Option<Extent3d>,

    start_texture: Option<Texture>,

    input_texture: Option<Texture>,
    input_temporal_buffer: Vec<Texture>,
    temporal_buffer_index: usize,

    output_texture: Option<Texture>,
    output_buffer: Option<Buffer>,

    pixels: Vec<u8>,

    textures: VecDeque<Vec<u8>>,
}

impl ComputeState {
    pub fn new() -> anyhow::Result<Self> {
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
            .ok_or(anyhow::anyhow!("Couldn't create the adapter"))?;

        if !adapter.features().contains(Features::TEXTURE_BINDING_ARRAY) {
            error!("Texture Binding Array Not supported");
        }

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("Device and queue"),
                    required_features: Features::TEXTURE_BINDING_ARRAY
                        | Features::STORAGE_RESOURCE_BINDING_ARRAY
                        | Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
                        | Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                    required_limits: Limits::default(),
                    memory_hints: MemoryHints::default(),
                },
                None,
            )
            .block_on()?;

        // let shader = device.create_shader_module(include_wgsl!("./shaders/shader.wgsl"));
        let shader = device.create_shader_module(include_wgsl!("./shaders/temporal_shader.wgsl"));

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Compute shader layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let bind_group_layout_2 = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Bind Group 2"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Pipeline layout"),
            bind_group_layouts: &[&bind_group_layout, &bind_group_layout_2],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("compute_main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            compute_pipeline,
            texture_bind_group: None,
            texture_bind_group_2: None,
            texture_bind_group_layout: bind_group_layout,
            texture_bind_group_layout_2: bind_group_layout_2,
            input_texture: None,
            input_temporal_buffer: Vec::new(),
            temporal_buffer_index: 0,
            start_texture: None,
            texture_dimensions: None,
            output_texture: None,
            output_buffer: None,
            pixels: Vec::new(),
            textures: VecDeque::new(),
        })
    }

    pub fn has_initial_frame(&self) -> bool {
        match self.input_texture {
            Some(_) => true,
            None => false,
        }
    }

    pub fn add_initial_texture(&mut self, width: u32, height: u32, frame_data: &[u8]) {
        let texture_size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let start_texture = self.device.create_texture(&TextureDescriptor {
            label: Some("Start Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Write the inital frame to the gpu
        self.queue.write_texture(
            start_texture.as_image_copy(),
            frame_data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        let mut temporal_buffer = Vec::new();
        for _ in 0..TEMPORAL_BUFFER_SIZE {
            let new_texture = self.device.create_texture(&TextureDescriptor {
                label: Some("temporal buffer texture"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });

            temporal_buffer.push(new_texture);
        }

        let output_texture = self.device.create_texture(&TextureDescriptor {
            label: Some("output texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let buffer_size =
            (padded_bytes_per_row(width) as u64 * height as u64) * std::mem::size_of::<u8>() as u64;

        let output_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("Output buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let texture_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(
                        &start_texture.create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(
                        &temporal_buffer[0].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(
                        &temporal_buffer[1].create_view(&TextureViewDescriptor::default()),
                    ),
                },
            ],
        });

        let texture_bind_group_2 = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Texture Bind Group 2"),
            layout: &self.texture_bind_group_layout_2,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(
                        &temporal_buffer[2].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(
                        &temporal_buffer[3].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(
                        &temporal_buffer[4].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(
                        &output_texture.create_view(&TextureViewDescriptor::default()),
                    ),
                },
            ],
        });

        self.texture_dimensions = Some(texture_size);
        self.start_texture = Some(start_texture);
        self.input_temporal_buffer = temporal_buffer;
        self.output_texture = Some(output_texture);
        self.texture_bind_group = Some(texture_bind_group);
        self.texture_bind_group_2 = Some(texture_bind_group_2);
        self.output_buffer = Some(output_buffer);
    }

    pub fn update_input_texture(&mut self, texture: &[u8]) {
        self.textures.push_back(texture.to_vec());

        if self.textures.len() > 5 {
            self.textures.pop_front();
        }

        for i in 0..self.textures.len() {
            self.queue.write_texture(
                self.input_temporal_buffer[i].as_image_copy(),
                &self.textures[i],
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.texture_dimensions.unwrap().width * 4),
                    rows_per_image: Some(self.texture_dimensions.unwrap().height),
                },
                self.texture_dimensions.unwrap(),
            );
        }

        self.temporal_buffer_index += 1;

        if self.temporal_buffer_index >= TEMPORAL_BUFFER_SIZE {
            self.temporal_buffer_index = 0;
        }
    }

    pub fn dispatch(&mut self) {
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Compute Command Encoder"),
            });

        {
            let (dispatch_width, dispatch_height) = compute_work_group_count(
                (
                    self.texture_dimensions.as_ref().unwrap().width,
                    self.texture_dimensions.as_ref().unwrap().height,
                ),
                (16, 16),
            );

            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, self.texture_bind_group.as_ref().unwrap(), &[]);
            compute_pass.set_bind_group(1, self.texture_bind_group_2.as_ref().unwrap(), &[]);
            compute_pass.dispatch_workgroups(dispatch_width, dispatch_height, 1);
        }

        let padded_bytes_per_row =
            padded_bytes_per_row(self.texture_dimensions.as_ref().unwrap().width);
        let unpadded_bytes_per_row = self.texture_dimensions.as_ref().unwrap().width * 4;

        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                aspect: TextureAspect::All,
                texture: self.output_texture.as_ref().unwrap(),
                mip_level: 0,
                origin: Origin3d::ZERO,
            },
            TexelCopyBufferInfo {
                buffer: self.output_buffer.as_ref().unwrap(),
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row as u32),
                    rows_per_image: Some(self.texture_dimensions.as_ref().unwrap().height as u32),
                },
            },
            self.texture_dimensions.unwrap(),
        );

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = self.output_buffer.as_ref().unwrap().slice(..);
        buffer_slice.map_async(MapMode::Read, |_| {});

        self.device.poll(Maintain::Wait);

        let padded_data = buffer_slice.get_mapped_range();

        self.pixels = vec![
            0;
            (unpadded_bytes_per_row * self.texture_dimensions.as_ref().unwrap().height)
                as usize
        ];

        for (padded, pixels) in padded_data.chunks_exact(padded_bytes_per_row).zip(
            self.pixels
                .chunks_exact_mut(unpadded_bytes_per_row as usize),
        ) {
            pixels.copy_from_slice(&padded[..unpadded_bytes_per_row as usize]);
        }

        drop(padded_data);

        self.output_buffer.as_ref().unwrap().unmap();
    }

    pub fn get_pixels(&self) -> Vec<u8> {
        self.pixels.clone()
    }

    pub fn save_output(&self) {
        if let Some(output_image) = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
            self.texture_dimensions.as_ref().unwrap().width,
            self.texture_dimensions.as_ref().unwrap().height,
            &self.pixels[..],
        ) {
            output_image
                .save("test_files/output.png")
                .expect("Failed to save image");
        }
    }
}

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
