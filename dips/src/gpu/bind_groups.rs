use std::{error::Error, fmt::Display, num::NonZeroU32};

// use log::*;

use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferDescriptor, BufferUsages,
    Device, Extent3d, PipelineLayout, PipelineLayoutDescriptor, Queue, ShaderStages,
    StorageTextureAccess, TexelCopyBufferLayout, Texture, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, TextureViewDescriptor,
};

// Constants
pub const TEMPORAL_BUFFER_SIZE: usize = 4;

// Error Structs
#[derive(Debug)]
pub struct BindGroupsAlreadyInitializedError;

impl Error for BindGroupsAlreadyInitializedError {
    fn description(&self) -> &str {
        "Bind Groups have already been initialized"
    }
}

impl Display for BindGroupsAlreadyInitializedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bind Groups have already been initialized")
    }
}

// Helper Functions
fn padded_bytes_per_row(width: u32) -> usize {
    let bytes_per_row = width as usize * 4;
    let padding = (256 - bytes_per_row % 256) % 256;
    bytes_per_row + padding
}

pub enum MainComputeBindGroups {
    Uninitialized(MainComputeBindGroupLayouts),
    Initialized(MainComputeBindGroupsContainer),
}

impl MainComputeBindGroups {
    /// Create new uninitialized bind groups
    pub fn new(device: &Device) -> Self {
        Self::Uninitialized(MainComputeBindGroupLayouts::new(device))
    }

    /// Initialize the bind groups with a set of textures and a starting texture
    pub fn initialize(
        main_compute_bind_groups: &mut Self,
        (device, queue): (&Device, &Queue),
        (width, height): (u32, u32),
        (starting_texture, temporal_textures): (&[u8], &[Vec<u8>]),
    ) -> Result<(), BindGroupsAlreadyInitializedError> {
        let new_main_compute_bind_groups: MainComputeBindGroups;

        match main_compute_bind_groups {
            Self::Uninitialized(main_compute_bind_group_layouts) => {
                new_main_compute_bind_groups =
                    Self::Initialized(MainComputeBindGroupsContainer::new(
                        main_compute_bind_group_layouts,
                        device,
                        width,
                        height,
                        starting_texture,
                        temporal_textures,
                        queue,
                    ));
            }
            Self::Initialized(_) => {
                return Err(BindGroupsAlreadyInitializedError);
            }
        }

        *main_compute_bind_groups = new_main_compute_bind_groups;

        Ok(())
    }

    /// Get a reference to the pipeline layout
    pub fn pipeline_layout(&self) -> Option<&PipelineLayout> {
        match self {
            Self::Uninitialized(main_compute_bind_group_layouts) => {
                Some(&main_compute_bind_group_layouts.pipeline_layout)
            }
            Self::Initialized(_) => None,
        }
    }
}

pub struct MainComputeBindGroupLayouts {
    start_texture_bind_group_layout: BindGroupLayout,
    temporal_textures_bind_group_layout: BindGroupLayout,
    output_texture_bind_group_layout: BindGroupLayout,
    pipeline_layout: PipelineLayout,
}

impl MainComputeBindGroupLayouts {
    pub fn new(device: &Device) -> Self {
        // Create the layout for the main compute input
        let start_texture_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("main compute start texture bind group layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            });

        // Create the layout for the main compute temporal textures
        let temporal_textures_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("main compute temporal textures bind group layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: NonZeroU32::new(TEMPORAL_BUFFER_SIZE as u32),
                }],
            });

        // Create the layout for the main compute output texture
        let output_texture_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("main compute output texture bind group layout"),
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

        // Create the pipeline layout for the main compute stage
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("main compute pipeline layout"),
            bind_group_layouts: &[
                &start_texture_bind_group_layout,
                &temporal_textures_bind_group_layout,
                &output_texture_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        Self {
            start_texture_bind_group_layout,
            temporal_textures_bind_group_layout,
            output_texture_bind_group_layout,
            pipeline_layout,
        }
    }
}

pub struct MainComputeBindGroupsContainer {
    pub texture_dimensions: Extent3d,

    pub start_texture_bind_group: BindGroup,
    start_texture: Texture,

    pub temporal_textures_bind_group: BindGroup,
    temporal_textures: Vec<Texture>,

    pub output_texture_bind_group: BindGroup,
    pub output_texture: Texture,
    pub output_texture_buffer: Buffer,
}

impl MainComputeBindGroupsContainer {
    pub fn new(
        main_bind_group_layouts: &MainComputeBindGroupLayouts,
        device: &Device,
        width: u32,
        height: u32,
        starting_texture: &[u8],
        textures: &[Vec<u8>],
        queue: &Queue,
    ) -> Self {
        let texture_dimensions = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        // Create the starting texture
        let start_texture = device.create_texture(&TextureDescriptor {
            label: Some("main compute start texture"),
            size: texture_dimensions,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            start_texture.as_image_copy(),
            starting_texture,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            texture_dimensions,
        );

        // Create the temporal textures
        let mut temporal_views = Vec::with_capacity(TEMPORAL_BUFFER_SIZE);
        let mut temporal_textures = Vec::with_capacity(TEMPORAL_BUFFER_SIZE);
        for frame_data in textures.iter() {
            let temporal_texture = device.create_texture(&TextureDescriptor {
                label: Some("main compute temporal texture"),
                size: texture_dimensions,
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
                texture_dimensions,
            );

            temporal_views.push(temporal_texture.create_view(&TextureViewDescriptor::default()));
            temporal_textures.push(temporal_texture);
        }

        // Create the output texture
        let output_texture = device.create_texture(&TextureDescriptor {
            label: Some("main compute output texture"),
            size: texture_dimensions,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Create the output buffer
        let output_texture_buffer = {
            let buffer_size = (padded_bytes_per_row(width) as u64 * height as u64)
                * std::mem::size_of::<u8>() as u64;

            device.create_buffer(&BufferDescriptor {
                label: Some("main compute output buffer"),
                size: buffer_size,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        };

        // Create the bind groups
        let (start_texture_bind_group, temporal_textures_bind_group, output_texture_bind_group) = {
            let start_texture_bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("main compute start texture bind group"),
                layout: &main_bind_group_layouts.start_texture_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(
                        &start_texture.create_view(&TextureViewDescriptor::default()),
                    ),
                }],
            });

            let temporal_view_refs: Vec<_> = temporal_views.iter().collect();
            let temporal_textures_bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("main compute temporal texture bind group"),
                layout: &main_bind_group_layouts.temporal_textures_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureViewArray(&temporal_view_refs),
                }],
            });

            let output_texture_bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("main compute output texture bind group"),
                layout: &main_bind_group_layouts.output_texture_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(
                        &output_texture.create_view(&TextureViewDescriptor::default()),
                    ),
                }],
            });

            (
                start_texture_bind_group,
                temporal_textures_bind_group,
                output_texture_bind_group,
            )
        };

        Self {
            texture_dimensions,

            start_texture_bind_group,
            start_texture,

            temporal_textures_bind_group,
            temporal_textures,

            output_texture_bind_group,
            output_texture,
            output_texture_buffer,
        }
    }

    pub fn set_start_texture(&mut self, input_texture: &[u8], queue: &Queue) {
        queue.write_texture(
            self.start_texture.as_image_copy(),
            input_texture,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.texture_dimensions.width * 4),
                rows_per_image: Some(self.texture_dimensions.height),
            },
            self.texture_dimensions,
        );
    }

    pub fn update_temporal_textures(&mut self, input_textures: &[Vec<u8>], queue: &Queue) {
        for (temporal_texture, input_texture) in
            self.temporal_textures.iter().zip(input_textures.iter())
        {
            queue.write_texture(
                temporal_texture.as_image_copy(),
                input_texture,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.texture_dimensions.width * 4),
                    rows_per_image: Some(self.texture_dimensions.height),
                },
                self.texture_dimensions,
            );
        }
    }
}

pub enum PreComputeBindGroups {
    // Create the PreComputeBindGroups with uninitialized
    Uninitialized(PreComputeBindGroupLayouts),
    // When we have enough textures we use this
    Initialized(PreComputeBindGroupsContainer),
}

impl PreComputeBindGroups {
    /// Create new uninitialized bind groups
    pub fn new(device: &Device) -> Self {
        Self::Uninitialized(PreComputeBindGroupLayouts::new(device))
    }

    /// Initialized the bind groups with a set of starting textures
    pub fn initialize(
        pre_compute_bind_groups: &mut Self,
        (device, queue): (&Device, &Queue),
        (width, height): (u32, u32),
        textures: &[Vec<u8>],
    ) -> Result<(), BindGroupsAlreadyInitializedError> {
        let new_pre_compute_bind_groups: PreComputeBindGroups;

        match pre_compute_bind_groups {
            Self::Uninitialized(pre_compute_bind_group_layouts) => {
                new_pre_compute_bind_groups =
                    Self::Initialized(PreComputeBindGroupsContainer::new(
                        pre_compute_bind_group_layouts,
                        device,
                        width,
                        height,
                        textures,
                        queue,
                    ));
            }
            Self::Initialized(_) => {
                return Err(BindGroupsAlreadyInitializedError);
            }
        }

        *pre_compute_bind_groups = new_pre_compute_bind_groups;

        Ok(())
    }

    /// Get a reference to the pipeline layout
    pub fn pipeline_layout(&self) -> Option<&PipelineLayout> {
        match self {
            Self::Uninitialized(pre_compute_bind_group_layouts) => {
                Some(&pre_compute_bind_group_layouts.pipeline_layout)
            }
            Self::Initialized(_) => None,
        }
    }
}

pub struct PreComputeBindGroupLayouts {
    start_textures_bind_group_layout: BindGroupLayout,
    output_texture_bind_group_layout: BindGroupLayout,
    pipeline_layout: PipelineLayout,
}

impl PreComputeBindGroupLayouts {
    pub fn new(device: &Device) -> Self {
        // Create the layout for the pre compute input
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
                    count: NonZeroU32::new(TEMPORAL_BUFFER_SIZE as u32),
                }],
            });

        // Create the layout for the pre compute output
        let output_texture_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Pre compute output texture bind group layout"),
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
                &output_texture_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        Self {
            start_textures_bind_group_layout,
            output_texture_bind_group_layout,
            pipeline_layout,
        }
    }
}

pub struct PreComputeBindGroupsContainer {
    pub texture_dimensions: Extent3d,

    // Pre compute start textures
    pub start_textures_bind_group: BindGroup,
    start_textures: Vec<Texture>,

    // Output of the pre compute state
    pub output_texture_bind_group: BindGroup,
    pub output_texture: Texture,
    pub output_texture_buffer: Buffer,
}

impl PreComputeBindGroupsContainer {
    pub fn new(
        pre_compute_bind_group_layouts: &PreComputeBindGroupLayouts,
        device: &Device,
        width: u32,
        height: u32,
        textures: &[Vec<u8>],
        queue: &Queue,
    ) -> Self {
        let texture_dimensions = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        // Create the array of starting textures
        let mut start_views = Vec::with_capacity(TEMPORAL_BUFFER_SIZE);
        let mut start_textures = Vec::with_capacity(TEMPORAL_BUFFER_SIZE);
        for frame_data in textures.iter() {
            let start_texture = device.create_texture(&TextureDescriptor {
                label: Some("pre compute Start Textures"),
                size: texture_dimensions,
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
                texture_dimensions,
            );

            start_views.push(start_texture.create_view(&TextureViewDescriptor::default()));
            start_textures.push(start_texture);
        }

        // Create the output texture
        let output_texture = device.create_texture(&TextureDescriptor {
            label: Some("pre compute output texture"),
            size: texture_dimensions,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Create the output buffer
        let output_texture_buffer = {
            let buffer_size = (padded_bytes_per_row(width) as u64 * height as u64)
                * std::mem::size_of::<u8>() as u64;

            device.create_buffer(&BufferDescriptor {
                label: Some("pre compute output buffer"),
                size: buffer_size,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        };

        // Create the bind groups with the texture views
        let (start_textures_bind_group, output_texture_bind_group) = {
            let start_view_refs: Vec<_> = start_views.iter().collect();
            let start_textures_bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Pre compute start textures bind group"),
                layout: &pre_compute_bind_group_layouts.start_textures_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureViewArray(&start_view_refs),
                }],
            });

            let output_texture_bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Pre compute outptu texture bind group"),
                layout: &pre_compute_bind_group_layouts.output_texture_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(
                        &output_texture.create_view(&TextureViewDescriptor::default()),
                    ),
                }],
            });

            (start_textures_bind_group, output_texture_bind_group)
        };

        Self {
            texture_dimensions,

            start_textures_bind_group,
            start_textures,

            output_texture_bind_group,
            output_texture,
            output_texture_buffer,
        }
    }
}
