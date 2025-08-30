use std::sync::Arc;

use anyhow::Result;
use gpu_controller::{GpuController, Surface};
use winit::{
    dpi::{PhysicalSize, Size},
    event_loop::ActiveEventLoop,
    window::Window,
};

pub struct DiPsWindow {
    pub(crate) window: Arc<Window>,
    pub(crate) surface: Arc<Surface<'static>>,
}

impl DiPsWindow {
    fn new(
        event_loop: &ActiveEventLoop,
        width: u32,
        height: u32,
        gpu_controller: &GpuController,
    ) -> Result<Self> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("DiPs")
                    .with_inner_size(Size::Physical(PhysicalSize { width, height })),
            )?,
        );

        let surface = gpu_controller.create_surface(window.clone())?;

        Ok(Self {
            window,
            surface: Arc::new(surface),
        })
    }
}
