use std::collections::{HashMap, VecDeque};

use bind_groups::{MainComputeBindGroups, PreComputeBindGroups};
use log::*;
use pollster::*;
use wgpu::{
    Backends, CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, DeviceDescriptor, Features, Instance, InstanceDescriptor,
    Limits, Maintain, MapMode, MemoryHints, Origin3d, PipelineCompilationOptions, PowerPreference,
    Queue, RequestAdapterOptionsBase, TexelCopyBufferInfo, TexelCopyBufferLayout,
    TexelCopyTextureInfo, TextureAspect, include_wgsl,
};

use crate::{ChromaFilter, DiPsFilter};

mod bind_groups;

// constants
const WORK_GROUP_WIDTH: u32 = 16;
const WORK_GROUP_HEIGHT: u32 = 16;

// helper functions
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

pub struct ComputeState {
    device: Device,
    queue: Queue,

    // Pre compute stage for creating the start texture
    pre_compute_pipeline: ComputePipeline,
    pre_compute_bind_groups: PreComputeBindGroups,

    // Main pipeline for compute DiPs
    compute_pipeline: ComputePipeline,
    main_compute_bind_groups: MainComputeBindGroups,

    pixels: Vec<u8>,

    textures: VecDeque<Vec<u8>>,

    starting_texture: Vec<u8>,
}

impl ComputeState {
    pub fn new(
        colorize: bool,
        spatial_window_size: i32,
        sensitivity: f32,
        filter_type: DiPsFilter,
        chroma_filter: ChromaFilter,
    ) -> anyhow::Result<Self> {
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
                        | Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                        | Features::UNIFORM_BUFFER_AND_STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING,
                    required_limits: Limits::default(),
                    memory_hints: MemoryHints::default(),
                },
                None,
            )
            .block_on()?;

        // These are the pipeline overrides to use
        let pipeline_compilation_options = {
            let mut hm = HashMap::new();
            hm.insert(String::from("0"), if colorize { 1.0 } else { 0.0 });
            hm.insert(String::from("1"), spatial_window_size as f64);
            hm.insert(String::from("2"), sensitivity as f64);
            hm.insert(String::from("3"), filter_type.into());
            hm.insert(String::from("4"), chroma_filter.into());
            hm
        };

        // Create the pre compute pipeline
        let (pre_compute_bind_groups, pre_compute_pipeline) = {
            let shader =
                device.create_shader_module(include_wgsl!("./shaders/pre_compute_shader.wgsl"));

            let pre_compute_bind_groups = PreComputeBindGroups::new(&device);

            let pre_compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Compute pipeline"),
                layout: Some(pre_compute_bind_groups.pipeline_layout().as_ref().unwrap()), // WARN need to handle if None
                module: &shader,
                entry_point: Some("pre_compute_main"),
                compilation_options: PipelineCompilationOptions {
                    constants: &pipeline_compilation_options,
                    ..Default::default()
                },
                cache: None,
            });

            (pre_compute_bind_groups, pre_compute_pipeline)
        };

        // Create the main compute pipeline
        let (main_compute_bind_groups, compute_pipeline) = {
            let shader = device.create_shader_module(include_wgsl!("./shaders/dips_shader.wgsl"));

            let bind_groups_container = MainComputeBindGroups::new(&device);

            let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Compute Pipeline"),
                layout: Some(&bind_groups_container.pipeline_layout().as_ref().unwrap()), // WARN need to handle if None
                module: &shader,
                entry_point: Some("compute_main"),
                compilation_options: PipelineCompilationOptions {
                    constants: &pipeline_compilation_options,
                    ..Default::default()
                },
                cache: None,
            });

            (bind_groups_container, compute_pipeline)
        };

        Ok(Self {
            device,
            queue,
            pre_compute_pipeline,
            pre_compute_bind_groups,
            compute_pipeline,
            main_compute_bind_groups,
            pixels: Vec::new(),
            textures: VecDeque::with_capacity(bind_groups::TEMPORAL_BUFFER_SIZE + 1), // NOTE this is done because it only deques once the texture buffer is greater than TEMPORAL_BUFFER_SIZE
            starting_texture: Vec::new(),
        })
    }

    /// Add textures to the compute state
    /// If there are TEMPORAL_BUFFER_SIZE textures added, then create the start texture
    /// and create the bind groups for the main compute pipeline
    pub fn add_texture(&mut self, width: u32, height: u32, frame_data: &[u8]) {
        self.textures.push_back(frame_data.to_vec());

        if self.textures.len() > bind_groups::TEMPORAL_BUFFER_SIZE {
            self.textures.pop_front();
        }

        if self.textures.len() == bind_groups::TEMPORAL_BUFFER_SIZE {
            match PreComputeBindGroups::initialize(
                &mut self.pre_compute_bind_groups,
                (&self.device, &self.queue),
                (width, height),
                self.textures.make_contiguous(),
            ) {
                Ok(_just_initialized) => {
                    // onces the precompute bindgroups have been initialized: create the pipeline,
                    // dispatch it, and create the starting texture
                    self.run_precompute_pipeline();
                }
                Err(_already_initialized) => {}
            }

            // FIXME: this api is really bad and should be fixed
            match MainComputeBindGroups::initialize(
                &mut self.main_compute_bind_groups,
                (&self.device, &self.queue),
                (width, height),
                (&self.starting_texture, self.textures.make_contiguous()),
            ) {
                Ok(_just_initialized) => {
                    if let MainComputeBindGroups::Initialized(bind_groups) =
                        &mut self.main_compute_bind_groups
                    {
                        bind_groups.set_start_texture(&self.starting_texture, &self.queue);
                    }
                }
                Err(_already_initialized) => {
                    // if it has already been initilized then update the textures
                    if let MainComputeBindGroups::Initialized(bind_groups) =
                        &mut self.main_compute_bind_groups
                    {
                        bind_groups.update_temporal_texture(frame_data, &self.queue);
                    }
                }
            }
        }
    }

    fn run_precompute_pipeline(&mut self) {
        if let PreComputeBindGroups::Initialized(bind_groups) = &self.pre_compute_bind_groups {
            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("pre compute command encoder"),
                });

            // Run the pipeline
            {
                let (dispatch_width, dispatch_height) = compute_work_group_count(
                    (
                        bind_groups.texture_dimensions.width,
                        bind_groups.texture_dimensions.height,
                    ),
                    (WORK_GROUP_WIDTH, WORK_GROUP_HEIGHT),
                );

                // Begin the pre compute pass
                let mut pre_compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("pre compute pass"),
                    timestamp_writes: None,
                });

                // Set the pipeline
                pre_compute_pass.set_pipeline(&self.pre_compute_pipeline);

                // Set the bind groups
                pre_compute_pass.set_bind_group(0, &bind_groups.start_textures_bind_group, &[]);
                pre_compute_pass.set_bind_group(1, &bind_groups.output_texture_bind_group, &[]);

                // Dispatch the work groups
                pre_compute_pass.dispatch_workgroups(dispatch_width, dispatch_height, 1);
            }

            // Copy the output texture over from the gpu
            let padded_bytes_per_row = padded_bytes_per_row(bind_groups.texture_dimensions.width);
            let unpadded_bytes_per_row = bind_groups.texture_dimensions.width * 4;

            encoder.copy_texture_to_buffer(
                TexelCopyTextureInfo {
                    aspect: TextureAspect::All,
                    texture: &bind_groups.output_texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                },
                TexelCopyBufferInfo {
                    buffer: &bind_groups.output_texture_buffer,
                    layout: TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row as u32),
                        rows_per_image: Some(bind_groups.texture_dimensions.height as u32),
                    },
                },
                bind_groups.texture_dimensions,
            );

            // Send the command encoder to the gpu
            self.queue.submit(Some(encoder.finish()));

            {
                // Now the buffer from the gpu is stored in output texture
                // We need to get it out and map it to make it usable as data
                let buffer_slice = bind_groups.output_texture_buffer.slice(..);
                buffer_slice.map_async(MapMode::Read, |_| {});
                self.device.poll(Maintain::Wait);

                let padded_data = buffer_slice.get_mapped_range();
                self.starting_texture = vec![
                    0;
                    (unpadded_bytes_per_row * bind_groups.texture_dimensions.height)
                        as usize
                ];

                for (padded, pixels) in padded_data.chunks_exact(padded_bytes_per_row).zip(
                    self.starting_texture
                        .chunks_exact_mut(unpadded_bytes_per_row as usize),
                ) {
                    pixels.copy_from_slice(&padded[..unpadded_bytes_per_row as usize]);
                }

                // deinitialize
                drop(padded_data);
                bind_groups.output_texture_buffer.unmap();
            }
        }
    }

    pub fn dispatch(&mut self) -> Option<Vec<u8>> {
        if let MainComputeBindGroups::Initialized(bind_groups) = &self.main_compute_bind_groups {
            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("main compute command encoder"),
                });

            // Run the pipeline
            {
                let (dispatch_width, dispatch_height) = compute_work_group_count(
                    (
                        bind_groups.texture_dimensions.width,
                        bind_groups.texture_dimensions.height,
                    ),
                    (WORK_GROUP_WIDTH, WORK_GROUP_HEIGHT),
                );

                // Begin the main compute pass
                let mut main_compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("main compute pass"),
                    timestamp_writes: None,
                });

                // Set the pipeilne
                main_compute_pass.set_pipeline(&self.compute_pipeline);

                // Set the bind groups
                main_compute_pass.set_bind_group(0, &bind_groups.start_texture_bind_group, &[]);
                main_compute_pass.set_bind_group(1, &bind_groups.temporal_textures_bind_group, &[]);
                main_compute_pass.set_bind_group(2, &bind_groups.output_texture_bind_group, &[]);

                // Dispatch the work groups
                main_compute_pass.dispatch_workgroups(dispatch_width, dispatch_height, 1);
            }

            // Copy the output texture over from the gpu
            let padded_bytes_per_row = padded_bytes_per_row(bind_groups.texture_dimensions.width);
            let unpadded_bytes_per_row = bind_groups.texture_dimensions.width * 4;

            encoder.copy_texture_to_buffer(
                TexelCopyTextureInfo {
                    aspect: TextureAspect::All,
                    texture: &bind_groups.output_texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                },
                TexelCopyBufferInfo {
                    buffer: &bind_groups.output_texture_buffer,
                    layout: TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row as u32),
                        rows_per_image: Some(bind_groups.texture_dimensions.height as u32),
                    },
                },
                bind_groups.texture_dimensions,
            );

            // Send the command encoder to the gpu
            self.queue.submit(Some(encoder.finish()));

            {
                // Now the buffer from the gpu is stored in output texture
                // We need to get it out and map it to make it usable as data
                let buffer_slice = bind_groups.output_texture_buffer.slice(..);
                buffer_slice.map_async(MapMode::Read, |_| {});
                self.device.poll(Maintain::Wait);

                let padded_data = buffer_slice.get_mapped_range();
                self.pixels = vec![
                    0;
                    (unpadded_bytes_per_row * bind_groups.texture_dimensions.height)
                        as usize
                ];

                for (padded, pixels) in padded_data.chunks_exact(padded_bytes_per_row).zip(
                    self.pixels
                        .chunks_exact_mut(unpadded_bytes_per_row as usize),
                ) {
                    pixels.copy_from_slice(&padded[..unpadded_bytes_per_row as usize]);
                }

                // deinitialize
                drop(padded_data);
                bind_groups.output_texture_buffer.unmap();
            }

            Some(self.pixels.clone())
        } else {
            None
        }
    }
}
