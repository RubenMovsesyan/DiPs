use std::sync::Arc;

use egui::Context;
use egui_wgpu::ScreenDescriptor;
use egui_winit::State;
use gpu_controller::{
    BindGroup, CommandEncoder, GpuController, LoadOp, Operations, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipeline, StoreOp, TextureFormat, TextureView,
};
use winit::{event::WindowEvent, window::Window};

use crate::window::DiPsWindow;

pub struct Renderer {
    pub pipeline: RenderPipeline,
    pub bind_group: BindGroup,
}

pub struct EguiRenderer {
    state: State,
    renderer: egui_wgpu::Renderer,
    frame_started: bool,

    gpu_controller: Arc<GpuController>,
}

impl EguiRenderer {
    pub fn context(&self) -> &Context {
        self.state.egui_ctx()
    }

    pub fn new(
        gpu_controller: Arc<GpuController>,
        output_color_format: TextureFormat,
        output_depth_format: Option<TextureFormat>,
        msaa_samples: u32,
        dips_window: &DiPsWindow,
    ) -> Self {
        let egui_context = Context::default();

        let egui_state = State::new(
            egui_context,
            egui::viewport::ViewportId::ROOT,
            &dips_window.window,
            Some(dips_window.window.scale_factor() as f32),
            None,
            Some(2 * 1024),
        );

        let egui_renderer = egui_wgpu::Renderer::new(
            gpu_controller.device(),
            output_color_format,
            output_depth_format,
            msaa_samples,
            true,
        );

        EguiRenderer {
            state: egui_state,
            renderer: egui_renderer,
            frame_started: false,

            gpu_controller,
        }
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) {
        _ = self.state.on_window_event(window, event);
    }

    pub fn ppp(&mut self, v: f32) {
        self.context().set_pixels_per_point(v);
    }

    pub fn begin_frame(&mut self, window: &Window) {
        let raw_input = self.state.take_egui_input(window);
        self.state.egui_ctx().begin_pass(raw_input);
        self.frame_started = true;
    }

    pub fn end_frame_and_draw(
        &mut self,
        window: &Window,
        encoder: &mut CommandEncoder,
        window_surface_view: &TextureView,
        screen_descriptor: ScreenDescriptor,
    ) {
        if !self.frame_started {
            panic!("begin_frame must be called before end_frame_and_draw can be called!");
        }

        self.ppp(screen_descriptor.pixels_per_point);

        let full_output = self.state.egui_ctx().end_pass();

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .state
            .egui_ctx()
            .tessellate(full_output.shapes, self.state.egui_ctx().pixels_per_point());

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer.update_texture(
                self.gpu_controller.device(),
                self.gpu_controller.queue(),
                *id,
                image_delta,
            );
        }

        self.renderer.update_buffers(
            self.gpu_controller.device(),
            self.gpu_controller.queue(),
            encoder,
            &tris,
            &screen_descriptor,
        );

        let render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("EGUI render pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        self.renderer.render(
            &mut render_pass.forget_lifetime(),
            &tris,
            &screen_descriptor,
        );

        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x);
        }

        self.frame_started = false;
    }
}
