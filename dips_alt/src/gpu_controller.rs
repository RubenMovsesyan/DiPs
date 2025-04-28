use std::rc::Rc;

use anyhow::{Result, anyhow};
use pollster::FutureExt;
use wgpu::{
    Adapter, Backends, Device, DeviceDescriptor, Features, Instance, InstanceDescriptor, Limits,
    MemoryHints, PowerPreference, Queue, RequestAdapterOptionsBase,
};

#[derive(Debug)]
pub struct GpuController {
    pub(crate) instance: Instance,
    pub(crate) adapter: Adapter,
    pub(crate) device: Rc<Device>,
    pub(crate) queue: Rc<Queue>,
}

impl GpuController {
    pub fn new() -> Result<Self> {
        // Initialize WGPU and attach it to a window if provided
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
            Err(err) => panic!("{err}"),
        };

        let (device, queue) = (Rc::new(device), Rc::new(queue));

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}
