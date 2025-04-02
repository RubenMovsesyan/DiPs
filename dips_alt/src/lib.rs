use std::{rc::Rc, sync::Arc};

use anyhow::{Result, anyhow};
use dips_compute::DiPsCompute;
use log::*;
use opencv::{
    core::{AlgorithmHint, VecN},
    highgui, imgproc,
    prelude::*,
    videoio::{self, VideoCaptureTraitConst},
};
use pollster::FutureExt;
use wgpu::{
    Adapter, Backends, Device, DeviceDescriptor, Features, Instance, InstanceDescriptor, Limits,
    MemoryHints, PowerPreference, PresentMode, Queue, RequestAdapterOptionsBase, Surface,
    SurfaceConfiguration, TextureUsages,
};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalSize, Size},
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    keyboard::Key,
    platform::modifier_supplement::KeyEventExtModifierSupplement,
    window::Window,
};

mod dips_compute;
mod utils;

const FRAME_COUNT: usize = 2;
// pub fn run_with_open_cv() -> Result<()> {
//     highgui::named_window("window", highgui::WINDOW_NORMAL)?;

//     // This is the main camera on the device, change index to access other
//     // device cameras
//     let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;

//     if !cam.is_opened()? {
//         panic!("Could not open camera");
//     }

//     let mut dips: Option<DiPsCompute> = None;
//     let mut frame = Mat::default();
//     let mut index: usize = 0;

//     loop {
//         cam.read(&mut frame)?;

//         let width = frame.rows();
//         let height = frame.cols();

//         if dips.is_none() {
//             dips = Some(DiPsCompute::new(
//                 FRAME_COUNT,
//                 width as u32,
//                 height as u32,
//                 None,
//             )?);
//             println!("w: {}, h: {}", width, height);
//         }

//         // Convert to rgba to be used in compute shader
//         let mut rgba_frame = Mat::default();

//         match imgproc::cvt_color(
//             &frame,
//             &mut rgba_frame,
//             imgproc::COLOR_BGR2RGBA,
//             0,
//             AlgorithmHint::ALGO_HINT_DEFAULT,
//         ) {
//             Ok(t) => t,
//             Err(err) => println!("Error: {:#?}", err),
//         }

//         let bytes = rgba_frame.data_bytes()?;

//         let new_frame_data = unsafe {
//             dips.as_mut().unwrap_unchecked().send_frame(
//                 &bytes,
//                 match index {
//                     FRAME_COUNT => Some(()),
//                     _ => None,
//                 },
//             )
//         };

//         if index < FRAME_COUNT + 1 {
//             index += 1;
//         }

//         let new_frame =
//             match Mat::new_rows_cols_with_bytes::<VecN<u8, 4>>(width, height, &new_frame_data) {
//                 Ok(t) => t,
//                 Err(err) => return Err(anyhow!(err)),
//             };

//         let mut output_frame = Mat::default();
//         imgproc::cvt_color(
//             &new_frame,
//             &mut output_frame,
//             imgproc::COLOR_RGBA2BGR,
//             0,
//             AlgorithmHint::ALGO_HINT_DEFAULT,
//         )?;

//         match highgui::imshow("window", &output_frame) {
//             Ok(t) => t,
//             Err(err) => println!("Error: {:#?}", err),
//         }

//         let key = highgui::wait_key(1)?;

//         // If pressing q then quit
//         if key == 'q' as i32 {
//             break;
//         } else if key == 's' as i32 {
//             index = 0;
//         }
//     }

//     Ok(())
// }

#[derive(Debug)]
struct DiPsWindow {
    window: Arc<Window>,
    surface: Arc<Surface<'static>>,
    surface_config: SurfaceConfiguration,
}

impl DiPsWindow {
    fn new(
        event_loop: &winit::event_loop::ActiveEventLoop,
        width: u32,
        height: u32,
        instance: &Instance,
        adapter: &Adapter,
        device: &Device,
        queue: &Queue,
    ) -> Result<Self> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("DiPs")
                    .with_inner_size(Size::Physical(PhysicalSize { width, height })),
            )?,
        );

        let surface = instance.create_surface(window.clone())?;
        let surface_capabilities = surface.get_capabilities(adapter);
        let size = window.inner_size();

        let surface_format = surface_capabilities
            .formats
            .iter()
            .find(|texture_format| texture_format.is_srgb())
            .copied()
            .unwrap_or(surface_capabilities.formats[0]);

        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::AutoNoVsync,
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(device, &surface_config);

        Ok(Self {
            window,
            surface: Arc::new(surface),
            surface_config,
        })
    }

    pub fn get_window(&self) -> Arc<Window> {
        self.window.clone()
    }
}

#[derive(Debug)]
pub struct DiPsApp {
    // window: Option<Arc<Window>>,
    dips_window: Option<DiPsWindow>,

    // WGPU
    device: Rc<Device>,
    queue: Rc<Queue>,
    instance: Instance,
    adapter: Adapter,

    compute: Option<DiPsCompute>,
    camera: videoio::VideoCapture,
    frame: Mat,
    index: usize,
}

impl DiPsApp {
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

        let camera = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;

        if !camera.is_opened()? {
            panic!("Camera Not Opened");
        }

        Ok(Self {
            dips_window: None,
            device,
            queue,
            instance,
            adapter,
            compute: None,
            camera,
            frame: Mat::default(),
            index: 0,
        })
    }

    fn initialize_dips(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) -> Result<()> {
        self.dips_window = Some(DiPsWindow::new(
            event_loop,
            self.camera.get(videoio::CAP_PROP_FRAME_WIDTH)? as u32,
            self.camera.get(videoio::CAP_PROP_FRAME_HEIGHT)? as u32,
            &self.instance,
            &self.adapter,
            &self.device,
            &self.queue,
        )?);

        Ok(())
    }

    /// This function runs the DiPs live camera portion of the app
    fn run_dips(&mut self) -> Result<()> {
        self.camera.read(&mut self.frame)?;

        let width = self.frame.rows();
        let height = self.frame.cols();

        if self.compute.is_none() {
            self.compute = Some(DiPsCompute::new(
                FRAME_COUNT,
                width as u32,
                height as u32,
                self.dips_window.as_ref(),
                self.device.clone(),
                self.queue.clone(),
            )?);
        }

        let mut rgba_frame = Mat::default();

        imgproc::cvt_color(
            &self.frame,
            &mut rgba_frame,
            imgproc::COLOR_BGR2RGBA,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;

        let bytes = rgba_frame.data_bytes()?;

        // Render the DiPs Frame
        _ = unsafe {
            self.compute.as_mut().unwrap_unchecked().send_frame(
                &bytes,
                match self.index {
                    FRAME_COUNT => Some(()),
                    _ => None,
                },
            )
        };

        if self.index <= FRAME_COUNT {
            self.index += 1;
        }

        Ok(())
    }
}

impl ApplicationHandler for DiPsApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        info!("DiPs Window Resumed");

        match self.initialize_dips(event_loop) {
            Ok(()) => {}
            Err(err) => error!("Failed to initialize DiPs: {err}"),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if self.dips_window.as_ref().unwrap().get_window().id() == window_id {
            match event {
                WindowEvent::CloseRequested => {
                    info!("Closing DiPs Window");
                    event_loop.exit();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    match event.key_without_modifiers().as_ref() {
                        Key::Character("s") => {
                            self.index = 0;
                        }
                        Key::Character("q") => {
                            event_loop.exit();
                        }
                        _ => {}
                    }
                }
                WindowEvent::RedrawRequested => match self.run_dips() {
                    Ok(()) => {}
                    Err(err) => error!("Encountered Error: {err}"),
                },
                _ => (),
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        self.dips_window
            .as_ref()
            .unwrap()
            .get_window()
            .request_redraw();
    }
}

pub fn run_dips_app() {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = DiPsApp::new().expect("Failed to create DiPs");
    _ = event_loop.run_app(&mut app);
}
