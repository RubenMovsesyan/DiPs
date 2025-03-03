use std::collections::VecDeque;

use bind_groups::{BindGroupsContainer, PreComputeBindGroupsContainer};
use log::*;
use pollster::*;
use wgpu::{
    Backends, CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, DeviceDescriptor, Features, Instance, InstanceDescriptor,
    Limits, Maintain, MapMode, MemoryHints, Origin3d, PipelineCompilationOptions, PowerPreference,
    Queue, RequestAdapterOptionsBase, TexelCopyBufferInfo, TexelCopyBufferLayout,
    TexelCopyTextureInfo, TextureAspect, include_wgsl,
};

mod bind_groups;

pub struct ComputeState {
    device: Device,
    queue: Queue,

    // Pre compute stage for creating the start texture
    pre_compute_pipeline: ComputePipeline,
    pre_compute_bind_groups_container: PreComputeBindGroupsContainer,

    // Main pipeline for compute DiPs
    compute_pipeline: ComputePipeline,
    bind_groups_container: BindGroupsContainer,

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

        // Create the pre compute pipeline
        let (pre_compute_bind_groups_container, pre_compute_pipeline) = {
            let shader = device.create_shader_module(todo!());

            let pre_compute_bind_groups_container = PreComputeBindGroupsContainer::new(&device);

            let pre_compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Compute pipeline"),
                layout: Some(&pre_compute_bind_groups_container.pipeline_layout),
                module: &shader,
                entry_point: Some("pre_compute_main"),
                compilation_options: PipelineCompilationOptions::default(),
                cache: None,
            });

            (pre_compute_bind_groups_container, pre_compute_pipeline)
        };

        // Create the main compute pipeline
        let (bind_groups_container, compute_pipeline) = {
            let shader = device.create_shader_module(include_wgsl!("./shaders/dips_shader.wgsl"));

            let bind_groups_container = BindGroupsContainer::new(&device);

            let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Compute Pipeline"),
                layout: Some(&bind_groups_container.pipeline_layout),
                module: &shader,
                entry_point: Some("compute_main"),
                compilation_options: PipelineCompilationOptions::default(),
                cache: None,
            });

            (bind_groups_container, compute_pipeline)
        };

        Ok(Self {
            device,
            queue,
            pre_compute_pipeline,
            pre_compute_bind_groups_container,
            compute_pipeline,
            bind_groups_container,
            pixels: Vec::new(),
            textures: VecDeque::new(),
        })
    }

    pub fn add_texture(&mut self, width: u32, height: u32, frame_data: &[u8]) {
        self.textures.push_back(frame_data.to_vec());

        if self.textures.len() > bind_groups::TEMPORAL_BUFFER_SIZE as usize {
            self.textures.pop_front();
        }

        if !self.bind_groups_container.initialized()
            && self.textures.len() == bind_groups::TEMPORAL_BUFFER_SIZE as usize
        {
            self.bind_groups_container.create_initial_bind_groups(
                width,
                height,
                self.textures.make_contiguous(),
                &self.device,
                &self.queue,
            );
        } else if self.bind_groups_container.initialized()
            && self.textures.len() == bind_groups::TEMPORAL_BUFFER_SIZE as usize
        {
            self.bind_groups_container
                .update_temporal_bind_groups(self.textures.make_contiguous(), &self.queue);
        }
    }

    pub fn can_dispatch(&self) -> bool {
        self.bind_groups_container.initialized()
    }

    pub fn dispatch(&mut self) {
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Compute Command Encoder"),
            });
        let dims = self
            .bind_groups_container
            .texture_dimensions
            .as_ref()
            .unwrap();

        {
            let (dispatch_width, dispatch_height) =
                compute_work_group_count((dims.width, dims.height), (16, 16));

            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(
                0,
                self.bind_groups_container
                    .start_textures_bind_group
                    .as_ref()
                    .unwrap(),
                &[],
            );
            compute_pass.set_bind_group(
                1,
                self.bind_groups_container
                    .temporal_textures_bind_group
                    .as_ref()
                    .unwrap(),
                &[],
            );
            compute_pass.set_bind_group(
                2,
                self.bind_groups_container
                    .output_texture_bind_group
                    .as_ref()
                    .unwrap(),
                &[],
            );

            compute_pass.dispatch_workgroups(dispatch_width, dispatch_height, 1);
        }

        let padded_bytes_per_row = padded_bytes_per_row(dims.width);
        let unpadded_bytes_per_row = dims.width * 4;

        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                aspect: TextureAspect::All,
                texture: self.bind_groups_container.output_texture.as_ref().unwrap(),
                mip_level: 0,
                origin: Origin3d::ZERO,
            },
            TexelCopyBufferInfo {
                buffer: self
                    .bind_groups_container
                    .output_texture_buffer
                    .as_ref()
                    .unwrap(),
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row as u32),
                    rows_per_image: Some(dims.height as u32),
                },
            },
            *dims,
        );

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = self
            .bind_groups_container
            .output_texture_buffer
            .as_ref()
            .unwrap()
            .slice(..);
        buffer_slice.map_async(MapMode::Read, |_| {});

        self.device.poll(Maintain::Wait);

        let padded_data = buffer_slice.get_mapped_range();

        self.pixels = vec![0; (unpadded_bytes_per_row * dims.height) as usize];

        for (padded, pixels) in padded_data.chunks_exact(padded_bytes_per_row).zip(
            self.pixels
                .chunks_exact_mut(unpadded_bytes_per_row as usize),
        ) {
            pixels.copy_from_slice(&padded[..unpadded_bytes_per_row as usize]);
        }

        drop(padded_data);

        self.bind_groups_container
            .output_texture_buffer
            .as_ref()
            .unwrap()
            .unmap();
    }

    pub fn get_pixels(&self) -> Vec<u8> {
        self.pixels.clone()
    }

    pub fn save_output(&self) {
        if let Some(output_image) = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
            self.bind_groups_container
                .texture_dimensions
                .as_ref()
                .unwrap()
                .width,
            self.bind_groups_container
                .texture_dimensions
                .as_ref()
                .unwrap()
                .height,
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
