use std::{error::Error, fs, path::Path, rc::Rc, sync::Arc};

use anyhow::{Result, anyhow};
use dips_compute::{ChromaFilter, DiPsCompute, DiPsProperties, Filter};
use egui_wgpu::ScreenDescriptor;
use gpu_controller::GpuController;
use gui::EguiRenderer;
use log::*;
use opencv::{
    core::{AlgorithmHint, VecN},
    highgui, imgproc,
    prelude::*,
    videoio::{self, VideoCaptureTraitConst},
};
use pollster::FutureExt;
use wgpu::{
    Adapter, Backends, CommandEncoderDescriptor, Device, DeviceDescriptor, Features, Instance,
    InstanceDescriptor, Limits, MemoryHints, PowerPreference, PresentMode,
    RequestAdapterOptionsBase, Surface, SurfaceConfiguration, SurfaceTexture, TextureUsages,
    TextureViewDescriptor,
};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalSize, Size},
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

mod dips_compute;
mod gpu_controller;
mod gui;
mod utils;

const FRAME_COUNT: usize = 2;

#[derive(Debug)]
pub enum Encoding {
    Uncompressed,
    Huffman,
}

impl Encoding {
    fn as_fourcc(&self) -> i32 {
        match self {
            Encoding::Uncompressed => {
                videoio::VideoWriter::fourcc('R', 'G', 'B', 'A').expect("Failed")
            }
            Encoding::Huffman => videoio::VideoWriter::fourcc('H', 'F', 'Y', 'U').expect("Failed"),
        }
    }
}

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
}

#[derive(Debug)]
pub struct DiPsApp {
    dips_window: Option<DiPsWindow>,

    // WGPU
    gpu_controller: GpuController,

    surface_texture: Option<SurfaceTexture>,

    compute: Option<DiPsCompute>,
    egui_renderer: Option<EguiRenderer>,
    camera: videoio::VideoCapture,
    frame: Mat,
    index: usize,

    // GUI variables
    colorize: bool,
    filter_type: Filter,
    chroma_filter: ChromaFilter,
    filter_sense: f32,
    spatial_window_size: u8,
}

impl DiPsApp {
    pub fn new() -> Result<Self> {
        let gpu_controller = GpuController::new()?;

        let camera = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;

        if !camera.is_opened()? {
            panic!("Camera Not Opened");
        }

        Ok(Self {
            dips_window: None,
            gpu_controller,
            compute: None,
            egui_renderer: None,
            camera,
            frame: Mat::default(),
            index: 0,
            surface_texture: None,
            colorize: true,
            filter_type: Filter::default(),
            chroma_filter: ChromaFilter::default(),
            filter_sense: 5.0,
            spatial_window_size: 1,
        })
    }

    fn initialize_dips(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) -> Result<()> {
        self.dips_window = Some(DiPsWindow::new(
            event_loop,
            self.camera.get(videoio::CAP_PROP_FRAME_WIDTH)? as u32,
            self.camera.get(videoio::CAP_PROP_FRAME_HEIGHT)? as u32,
            &self.gpu_controller.instance,
            &self.gpu_controller.adapter,
            &self.gpu_controller.device,
        )?);

        self.egui_renderer = Some(EguiRenderer::new(
            self.gpu_controller.device.clone(),
            self.gpu_controller.queue.clone(),
            self.dips_window.as_ref().unwrap().surface_config.format,
            None,
            1,
            &self.dips_window.as_ref().unwrap(),
        ));

        Ok(())
    }

    /// This function runs the DiPs live camera portion of the app
    fn run_dips(&mut self) -> Result<()> {
        self.camera.read(&mut self.frame)?;

        let width = self.frame.rows();
        let height = self.frame.cols();

        // println!("w: {} h: {}", width, height);

        if self.compute.is_none() {
            self.compute = Some(DiPsCompute::new(
                FRAME_COUNT,
                width as u32,
                height as u32,
                self.dips_window.as_ref(),
                self.gpu_controller.device.clone(),
                self.gpu_controller.queue.clone(),
                DiPsProperties::default(),
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
                self.surface_texture.as_ref(),
            )
        };

        if self.index <= FRAME_COUNT {
            self.index += 1;
        }

        Ok(())
    }

    fn run_egui(&mut self) -> Result<()> {
        if let Some(renderer) = self.egui_renderer.as_mut() {
            let screen_descriptor = ScreenDescriptor {
                size_in_pixels: [
                    self.dips_window.as_ref().unwrap().surface_config.width,
                    self.dips_window.as_ref().unwrap().surface_config.height,
                ],
                pixels_per_point: self.dips_window.as_ref().unwrap().window.scale_factor() as f32,
            };

            let surface_texture = self.surface_texture.as_ref().unwrap();

            let surface_view = surface_texture
                .texture
                .create_view(&TextureViewDescriptor::default());

            let mut encoder =
                self.gpu_controller
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("GUI Command Encoder"),
                    });

            renderer.begin_frame(&self.dips_window.as_ref().unwrap().window);

            egui::Window::new("DiPs Controls")
                .resizable(true)
                .vscroll(true)
                .default_open(false)
                .show(renderer.context(), |ui| {
                    let redip = |color: bool,
                                 filter: Filter,
                                 chroma: ChromaFilter,
                                 filter_sense: f32,
                                 spatial_window_size: u8| {
                        DiPsCompute::new(
                            FRAME_COUNT,
                            self.dips_window
                                .as_ref()
                                .unwrap()
                                .window
                                .inner_size()
                                .height,
                            self.dips_window.as_ref().unwrap().window.inner_size().width,
                            self.dips_window.as_ref(),
                            self.gpu_controller.device.clone(),
                            self.gpu_controller.queue.clone(),
                            DiPsProperties {
                                colorize: color,
                                filter_type: filter,
                                chroma_filter: chroma,
                                sigmoid_horizontal_scalar: filter_sense,
                                window_size: spatial_window_size,
                            },
                        )
                        .expect("Failed to redip")
                    };

                    // This is the button to take a snapshot and reset the initial frame
                    if ui.button("SnapShot").clicked() {
                        self.index = 0;
                    }

                    // This is the checkbox for Colorizing the output
                    if ui.checkbox(&mut self.colorize, "Colorize").changed() {
                        self.index = 0;
                        self.compute = Some(redip(
                            self.colorize,
                            self.filter_type,
                            self.chroma_filter,
                            self.filter_sense,
                            self.spatial_window_size,
                        ));
                    }

                    // This is the combo box to select the sensitivity filter type to be used during the DiPs
                    // Sigmoid
                    // Inverse Sigmoid
                    egui::ComboBox::from_label("Filter Type")
                        .selected_text(match self.filter_type {
                            Filter::Sigmoid => "Sigmoid",
                            Filter::InverseSigmoid => "Inverse Sigmoid",
                        })
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(&mut self.filter_type, Filter::Sigmoid, "Sigmoid")
                                .clicked()
                            {
                                self.index = 0;
                                self.compute = Some(redip(
                                    self.colorize,
                                    self.filter_type,
                                    self.chroma_filter,
                                    self.filter_sense,
                                    self.spatial_window_size,
                                ));
                            };

                            if ui
                                .selectable_value(
                                    &mut self.filter_type,
                                    Filter::InverseSigmoid,
                                    "Inverse Sigmoid",
                                )
                                .clicked()
                            {
                                self.index = 0;
                                self.compute = Some(redip(
                                    self.colorize,
                                    self.filter_type,
                                    self.chroma_filter,
                                    self.filter_sense,
                                    self.spatial_window_size,
                                ));
                            };
                        });

                    // This is the slider to choose a horizontal scalar value for the sensitivity filter
                    if ui
                        .add(
                            egui::Slider::new(&mut self.filter_sense, 1.0..=10.0)
                                .text("Filter Sensitivity"),
                        )
                        .drag_stopped()
                    {
                        self.index = 0;
                        self.compute = Some(redip(
                            self.colorize,
                            self.filter_type,
                            self.chroma_filter,
                            self.filter_sense,
                            self.spatial_window_size,
                        ));
                    };

                    // This is the combo box to select which chroma filter to use
                    // Default is all channels
                    egui::ComboBox::from_label("Chroma Filter")
                        .selected_text(match self.chroma_filter {
                            ChromaFilter::All => "All",
                            ChromaFilter::Red => "Red",
                            ChromaFilter::Green => "Green",
                            ChromaFilter::Blue => "Blue",
                        })
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(&mut self.chroma_filter, ChromaFilter::All, "All")
                                .clicked()
                            {
                                self.index = 0;
                                self.compute = Some(redip(
                                    self.colorize,
                                    self.filter_type,
                                    self.chroma_filter,
                                    self.filter_sense,
                                    self.spatial_window_size,
                                ));
                            }

                            if ui
                                .selectable_value(&mut self.chroma_filter, ChromaFilter::Red, "Red")
                                .clicked()
                            {
                                self.index = 0;
                                self.compute = Some(redip(
                                    self.colorize,
                                    self.filter_type,
                                    self.chroma_filter,
                                    self.filter_sense,
                                    self.spatial_window_size,
                                ));
                            }

                            if ui
                                .selectable_value(
                                    &mut self.chroma_filter,
                                    ChromaFilter::Green,
                                    "Green",
                                )
                                .clicked()
                            {
                                self.index = 0;
                                self.compute = Some(redip(
                                    self.colorize,
                                    self.filter_type,
                                    self.chroma_filter,
                                    self.filter_sense,
                                    self.spatial_window_size,
                                ));
                            }

                            if ui
                                .selectable_value(
                                    &mut self.chroma_filter,
                                    ChromaFilter::Blue,
                                    "Blue",
                                )
                                .clicked()
                            {
                                self.index = 0;
                                self.compute = Some(redip(
                                    self.colorize,
                                    self.filter_type,
                                    self.chroma_filter,
                                    self.filter_sense,
                                    self.spatial_window_size,
                                ));
                            }
                        });

                    // This is the slider to choose a spatial window size
                    if ui
                        .add(
                            egui::Slider::new(&mut self.spatial_window_size, 1..=7)
                                .text("Spatial Window Filtering")
                                .step_by(2.0),
                        )
                        .drag_stopped()
                    {
                        self.index = 0;
                        self.compute = Some(redip(
                            self.colorize,
                            self.filter_type,
                            self.chroma_filter,
                            self.filter_sense,
                            self.spatial_window_size,
                        ));
                    };
                });

            renderer.end_frame_and_draw(
                &self.dips_window.as_ref().unwrap().window,
                &mut encoder,
                &surface_view,
                screen_descriptor,
            );

            renderer.queue.submit(Some(encoder.finish()));
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
        if self.dips_window.as_ref().unwrap().window.id() == window_id {
            match event {
                WindowEvent::CloseRequested => {
                    info!("Closing DiPs Window");
                    event_loop.exit();
                }
                WindowEvent::RedrawRequested => {
                    self.surface_texture = Some(
                        self.dips_window
                            .as_ref()
                            .unwrap()
                            .surface
                            .get_current_texture()
                            .expect("Failed to get surface texture"),
                    );

                    match self.run_dips() {
                        Ok(()) => {}
                        Err(err) => error!("Encountered Error: {err}"),
                    }

                    match self.run_egui() {
                        Ok(()) => {}
                        Err(err) => error!("Encountered Error: {err}"),
                    }

                    self.surface_texture.take().unwrap().present();
                }
                WindowEvent::KeyboardInput { .. }
                | WindowEvent::MouseInput { .. }
                | WindowEvent::CursorMoved { .. }
                | WindowEvent::MouseWheel { .. } => {
                    if let Some(renderer) = self.egui_renderer.as_mut() {
                        renderer.handle_input(&self.dips_window.as_ref().unwrap().window, &event);
                    }
                }
                _ => (),
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        self.dips_window.as_ref().unwrap().window.request_redraw();
    }
}

pub fn run_dips_app() -> anyhow::Result<()> {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = DiPsApp::new().expect("Failed to create DiPs");
    _ = event_loop.run_app(&mut app);

    Ok(())
}

pub fn run_dips_on_file<P>(
    path: P,
    output: P,
    encoding: Encoding,
    refresh_markers: Vec<usize>,
) -> Result<()>
where
    P: AsRef<Path>,
{
    let gpu_controller = GpuController::new()?;

    let mut overall_frame: usize = 0;
    let mut index: usize = 0;

    highgui::named_window("DiPs", highgui::WINDOW_NORMAL)?;

    let mut file_stream = videoio::VideoCapture::from_file(
        path.as_ref().as_os_str().to_str().unwrap(),
        videoio::CAP_ANY,
    )?;

    let fps = file_stream.get(videoio::CAP_PROP_FPS)?;

    // let fourcc = videoio::VideoWriter::fourcc('R', 'G', 'B', 'A')?;
    let fourcc = encoding.as_fourcc();
    let mut output_stream = None;

    if !file_stream.is_opened()? {
        panic!("Failed to open file");
    }

    let mut frame = Mat::default();
    let mut compute_state: Option<DiPsCompute> = None;

    loop {
        if !file_stream.read(&mut frame)? {
            break;
        }

        let pts = file_stream.get(videoio::CAP_PROP_PTS)?;
        let dts = file_stream.get(videoio::CAP_PROP_DTS_DELAY)?;

        let width = frame.rows();
        let height = frame.cols();

        if compute_state.is_none() {
            compute_state = Some(DiPsCompute::new(
                FRAME_COUNT,
                width as u32,
                height as u32,
                None,
                gpu_controller.device.clone(),
                gpu_controller.queue.clone(),
                DiPsProperties::default(),
            )?);
        }

        if output_stream.is_none() {
            output_stream = Some(videoio::VideoWriter::new(
                output.as_ref().as_os_str().to_str().unwrap(),
                fourcc,
                fps,
                opencv::core::Size::new(height, width),
                true,
            )?);
        }

        let mut rgba_frame = Mat::default();

        imgproc::cvt_color(
            &frame,
            &mut rgba_frame,
            imgproc::COLOR_BGR2RGBA,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;

        let bytes = rgba_frame.data_bytes()?;

        let new_frame_data = unsafe {
            compute_state.as_mut().unwrap_unchecked().send_frame(
                &bytes,
                match index {
                    FRAME_COUNT => Some(()),
                    _ => None,
                },
                None,
            )
        };

        let new_frame =
            match Mat::new_rows_cols_with_bytes::<VecN<u8, 4>>(width, height, &new_frame_data) {
                Ok(t) => t,
                Err(err) => {
                    println!("Error: {:#?}", err);
                    return Err(anyhow::Error::new(err));
                }
            };

        let mut output_frame = Mat::default();
        imgproc::cvt_color(
            &new_frame,
            &mut output_frame,
            imgproc::COLOR_RGBA2BGR,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;

        if index <= FRAME_COUNT {
            index += 1;
        }

        overall_frame += 1;

        if refresh_markers.contains(&overall_frame) {
            index = 0;
        }

        if let Some(stream) = output_stream.as_mut() {
            print!("\rFrame: {}", overall_frame);
            stream.set(videoio::VIDEOWRITER_PROP_PTS, pts)?;
            stream.set(videoio::VIDEOWRITER_PROP_DTS_DELAY, dts)?;
            stream.write(&output_frame)?;
        }

        match highgui::imshow("DiPs", &output_frame) {
            Ok(_) => (),
            Err(err) => println!("Error: {:#?}", err),
        }
    }

    if let Some(mut writer) = output_stream.take() {
        writer.release()?;
    }

    Ok(())
}

pub fn custom_dips_on_files<P>(config_path: P, data_dir: P, output: P) -> Result<()>
where
    P: AsRef<Path>,
{
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

    let mut index: usize = 0;

    highgui::named_window("DiPs", highgui::WINDOW_NORMAL)?;

    // let mut file_stream = videoio::VideoCapture::from_file(
    //     path.as_ref().as_os_str().to_str().unwrap(),
    //     videoio::CAP_ANY,
    // )?;

    let fourcc = videoio::VideoWriter::fourcc('R', 'G', 'B', 'A')?;
    let mut output_stream = None;

    // if !file_stream.is_opened()? {
    //     panic!("Failed to open file");
    // }

    // let mut frame = Mat::default();
    let mut compute_state: Option<DiPsCompute> = None;

    let mut paths = fs::read_dir(data_dir)
        .unwrap()
        .map(|r| r.unwrap())
        .collect::<Vec<_>>();

    paths.sort_by_key(|dir| {
        dir.path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
            .replace(&['D', 'a', 't', '_', '.', 'd'][..], "")
            .parse::<u32>()
            .unwrap_or(0)
    });

    let mut split_index = 0;
    while paths[split_index]
        .file_name()
        .into_string()
        .unwrap()
        .chars()
        .nth(0)
        .as_ref()
        .unwrap()
        != &'D'
    {
        split_index += 1;
    }

    paths = paths.split_off(split_index);

    // println!("{:#?}", paths);

    let width = 273;
    let height = 640;

    for file_path in paths.iter() {
        // let frame_available = file_stream.read(&mut frame)?;
        let file_data = &fs::read(file_path.path()).unwrap()[4..];
        // let conv_data: &[i32] = bytemuck::cast_slice(file_data);
        // println!("{}", file_data.len());
        let frame = Mat::new_rows_cols_with_bytes::<VecN<u8, 4>>(width, height, file_data)?;
        // let width = frame.rows();
        // let height = frame.cols();

        if compute_state.is_none() {
            compute_state = Some(DiPsCompute::new(
                FRAME_COUNT,
                width as u32,
                height as u32,
                None,
                device.clone(),
                queue.clone(),
                DiPsProperties::default(),
            )?);
        }

        if output_stream.is_none() {
            output_stream = Some(videoio::VideoWriter::new(
                output.as_ref().as_os_str().to_str().unwrap(),
                fourcc,
                5.0,
                opencv::core::Size::new(height, width),
                true,
            )?);
        }

        // let mut rgba_frame = Mat::default();

        // imgproc::cvt_color(
        //     &frame,
        //     &mut rgba_frame,
        //     imgproc::COLOR_BGR2RGBA,
        //     0,
        //     AlgorithmHint::ALGO_HINT_DEFAULT,
        // )?;

        // let bytes = rgba_frame.data_bytes()?;
        let bytes = frame.data_bytes()?;
        // println!("len: {}", bytes.len());

        let new_frame_data = unsafe {
            compute_state.as_mut().unwrap_unchecked().send_frame(
                &bytes,
                match index {
                    FRAME_COUNT => Some(()),
                    _ => None,
                },
                None,
            )
        };

        let new_frame =
            match Mat::new_rows_cols_with_bytes::<VecN<u8, 4>>(width, height, &new_frame_data) {
                Ok(t) => t,
                Err(err) => {
                    println!("Error: {:#?}", err);
                    return Err(anyhow::Error::new(err));
                }
            };

        let mut output_frame = Mat::default();
        imgproc::cvt_color(
            &new_frame,
            &mut output_frame,
            imgproc::COLOR_RGBA2BGR,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;

        if index <= FRAME_COUNT {
            index += 1;
        }

        println!("{:#?}", &output_frame);

        match unsafe {
            output_stream
                .as_mut()
                .unwrap_unchecked()
                .write(&output_frame)
        } {
            Ok(_) => (),
            // Ok(false) => error!("Failed to write frame to video file"),
            Err(e) => error!("Error Writing frame: {}", e),
        }

        match highgui::imshow("DiPs", &output_frame) {
            Ok(_) => (),
            Err(err) => println!("Error: {:#?}", err),
        }

        let key = highgui::wait_key(1)?;

        if key == 'q' as i32 {
            break;
        }
    }

    // output_stream.as_mut().unwrap().release()?;
    if let Some(mut writer) = output_stream {
        writer.release()?;
    }

    Ok(())
}
