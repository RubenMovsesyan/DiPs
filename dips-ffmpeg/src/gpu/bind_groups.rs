use std::num::NonZeroU32;

// use log::*;

use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferDescriptor, BufferUsages,
    Device, Extent3d, PipelineLayout, PipelineLayoutDescriptor, Queue, ShaderStages,
    StorageTextureAccess, TexelCopyBufferLayout, Texture, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, TextureViewDescriptor,
};

pub const TEMPORAL_BUFFER_SIZE: u32 = 4;

pub struct BindGroupsContainer {
    pub texture_dimensions: Option<Extent3d>,

    start_textures_bind_group_layout: BindGroupLayout,
    pub start_textures_bind_group: Option<BindGroup>,
    start_textures: Vec<Texture>,

    temporal_textures_bind_group_layout: BindGroupLayout,
    pub temporal_textures_bind_group: Option<BindGroup>,
    temporal_textures: Vec<Texture>,

    output_texture_bind_group_layout: BindGroupLayout,
    pub output_texture_bind_group: Option<BindGroup>,
    pub output_texture: Option<Texture>,
    pub output_texture_buffer: Option<Buffer>,

    pub pipeline_layout: PipelineLayout,
}

impl BindGroupsContainer {
    pub fn initialized(&self) -> bool {
        match self.texture_dimensions {
            Some(_) => true,
            None => false,
        }
    }

    pub fn new(device: &Device) -> Self {
        // Create the bind group layouts needed
        let start_textures_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Start textures bind group layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: NonZeroU32::new(TEMPORAL_BUFFER_SIZE),
                }],
            });

        let temporal_textures_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Temporal textures bind group layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: NonZeroU32::new(TEMPORAL_BUFFER_SIZE),
                }],
            });

        let output_texture_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Output textures bind group layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            });

        // Create the pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Pipeline layout"),
            bind_group_layouts: &[
                &start_textures_bind_group_layout,
                &temporal_textures_bind_group_layout,
                &output_texture_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        Self {
            texture_dimensions: None,

            start_textures_bind_group_layout,
            start_textures_bind_group: None,
            start_textures: Vec::new(),

            temporal_textures_bind_group_layout,
            temporal_textures_bind_group: None,
            temporal_textures: Vec::new(),

            output_texture_bind_group_layout,
            output_texture_bind_group: None,
            output_texture: None,
            output_texture_buffer: None,

            pipeline_layout,
        }
    }

    pub fn create_initial_bind_groups(
        &mut self,
        width: u32,
        height: u32,
        textures: &[Vec<u8>],
        device: &Device,
        queue: &Queue,
    ) {
        let texture_size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        self.texture_dimensions = Some(texture_size);

        // Create the array of starting textures
        let mut start_views = Vec::with_capacity(TEMPORAL_BUFFER_SIZE as usize);
        for frame_data in textures.iter() {
            let start_texture = device.create_texture(&TextureDescriptor {
                label: Some("Start Texture"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });

            queue.write_texture(
                start_texture.as_image_copy(),
                &frame_data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                texture_size,
            );

            start_views.push(start_texture.create_view(&TextureViewDescriptor::default()));
            self.start_textures.push(start_texture);
        }

        // Create the array of temporal textures
        let mut temporal_views = Vec::with_capacity(TEMPORAL_BUFFER_SIZE as usize);
        for frame_data in textures.iter() {
            let temporal_texture = device.create_texture(&TextureDescriptor {
                label: Some("Temporal Texture"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });

            queue.write_texture(
                temporal_texture.as_image_copy(),
                &frame_data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                texture_size,
            );

            temporal_views.push(temporal_texture.create_view(&TextureViewDescriptor::default()));
            self.temporal_textures.push(temporal_texture);
        }

        // Create the output texture
        let output_texture = device.create_texture(&TextureDescriptor {
            label: Some("output texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Create the output buffer to read from
        let buffer_size =
            (padded_bytes_per_row(width) as u64 * height as u64) * std::mem::size_of::<u8>() as u64;

        let output_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Output buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        self.output_texture_buffer = Some(output_buffer);

        // Create the bind groups now that we have created the textures
        let start_view_refs: Vec<_> = start_views.iter().collect();
        let start_textures_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Start Textures Bind Group"),
            layout: &self.start_textures_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureViewArray(&start_view_refs),
            }],
        });

        let temporal_view_refs: Vec<_> = temporal_views.iter().collect();
        let temporal_textures_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Temporal Textures Bind Group"),
            layout: &self.temporal_textures_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureViewArray(&temporal_view_refs),
            }],
        });

        let output_texture_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Output Texture Bind Group"),
            layout: &self.output_texture_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(
                    &output_texture.create_view(&TextureViewDescriptor::default()),
                ),
            }],
        });

        // Set the bind groups
        self.output_texture = Some(output_texture);
        self.start_textures_bind_group = Some(start_textures_bind_group);
        self.temporal_textures_bind_group = Some(temporal_textures_bind_group);
        self.output_texture_bind_group = Some(output_texture_bind_group);
    }

    pub fn update_temporal_bind_groups(&mut self, textures: &[Vec<u8>], queue: &Queue) {
        let texture_size = match self.texture_dimensions {
            Some(size) => size,
            None => panic!("Texture size not set"),
        };

        for i in 0..TEMPORAL_BUFFER_SIZE as usize {
            queue.write_texture(
                self.temporal_textures[i].as_image_copy(),
                &textures[i],
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(texture_size.width * 4),
                    rows_per_image: Some(texture_size.height),
                },
                texture_size,
            );
        }
    }
}

fn padded_bytes_per_row(width: u32) -> usize {
    let bytes_per_row = width as usize * 4;
    let padding = (256 - bytes_per_row % 256) % 256;
    bytes_per_row + padding
}
