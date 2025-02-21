use log::*;
use pollster::*;
use wgpu::{
    include_wgsl, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource,
    Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor,
    ComputePipeline, ComputePipelineDescriptor, Device, Extent3d, Instance, InstanceDescriptor,
    Maintain, MapMode, Origin3d, PipelineCompilationOptions, PowerPreference, Queue,
    RequestAdapterOptionsBase, TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo,
    Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureViewDescriptor,
};

pub struct ComputeState {
    device: Device,
    queue: Queue,

    compute_pipeline: ComputePipeline,
    texture_bind_group: Option<BindGroup>,

    input_texture: Option<Texture>,
    input_texture_dimensions: Option<Extent3d>,

    output_texture: Option<Texture>,
    output_buffer: Option<Buffer>,
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

        let (device, queue) = adapter
            .request_device(&Default::default(), None)
            .block_on()?;

        let shader = device.create_shader_module(include_wgsl!("./shaders/shader.wgsl"));

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: None,
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
            input_texture: None,
            input_texture_dimensions: None,
            output_texture: None,
            output_buffer: None,
        })
    }

    pub fn has_initial_frame(&self) -> bool {
        match self.input_texture {
            Some(_) => true,
            None => false,
        }
    }

    pub fn add_initial_texture(&mut self, width: u32, height: u32) {
        let texture_size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let input_texture = self.device.create_texture(&TextureDescriptor {
            label: Some("input texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm, // WARN: This might need to be changed to SRBB
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

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
            label: Some("Texture Bind group"),
            layout: &self.compute_pipeline.get_bind_group_layout(0),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(
                        &input_texture.create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(
                        &output_texture.create_view(&TextureViewDescriptor::default()),
                    ),
                },
            ],
        });

        self.input_texture_dimensions = Some(texture_size);
        self.input_texture = Some(input_texture);
        self.output_texture = Some(output_texture);
        self.texture_bind_group = Some(texture_bind_group);
        self.output_buffer = Some(output_buffer);
    }

    pub fn update_input_texture(&self, texture: &[u8]) {
        self.queue.write_texture(
            self.input_texture.as_ref().unwrap().as_image_copy(),
            texture,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.input_texture_dimensions.unwrap().width * 4),
                rows_per_image: Some(self.input_texture_dimensions.unwrap().height),
            },
            self.input_texture_dimensions.unwrap(),
        );
    }

    pub fn dispatch(&self) {
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Compute Command Encoder"),
            });

        {
            let (dispatch_width, dispatch_height) = compute_work_group_count(
                (
                    self.input_texture_dimensions.as_ref().unwrap().width,
                    self.input_texture_dimensions.as_ref().unwrap().height,
                ),
                (16, 16),
            );

            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, self.texture_bind_group.as_ref().unwrap(), &[]);
            compute_pass.dispatch_workgroups(dispatch_width, dispatch_height, 1);
        }

        let padded_bytes_per_row =
            padded_bytes_per_row(self.input_texture_dimensions.as_ref().unwrap().width);
        let unpadded_bytes_per_row = self.input_texture_dimensions.as_ref().unwrap().width * 4;

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
                    rows_per_image: Some(
                        self.input_texture_dimensions.as_ref().unwrap().height as u32,
                    ),
                },
            },
            self.input_texture_dimensions.unwrap(),
        );

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = self.output_buffer.as_ref().unwrap().slice(..);
        buffer_slice.map_async(MapMode::Read, |_| {});

        self.device.poll(Maintain::Wait);

        let padded_data = buffer_slice.get_mapped_range();

        let mut pixels: Vec<u8> = vec![
            0;
            (unpadded_bytes_per_row * self.input_texture_dimensions.as_ref().unwrap().height)
                as usize
        ];

        for (padded, pixels) in padded_data
            .chunks_exact(padded_bytes_per_row)
            .zip(pixels.chunks_exact_mut(unpadded_bytes_per_row as usize))
        {
            pixels.copy_from_slice(&padded[..unpadded_bytes_per_row as usize]);
        }

        if let Some(output_image) = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
            self.input_texture_dimensions.as_ref().unwrap().width,
            self.input_texture_dimensions.as_ref().unwrap().height,
            &pixels[..],
        ) {
            output_image
                .save("test_files/output.png")
                .expect("Failed to save image");
        }

        drop(padded_data);

        self.output_buffer.as_ref().unwrap().unmap();
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
