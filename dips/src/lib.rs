use std::{
    error::Error,
    fmt::Display,
    sync::{Arc, Mutex},
};

use gpu::ComputeState;
// Logging
// use log::*;

mod frame_extractor;
mod gpu;

use frame_extractor::*;

// Type alias for the callback function
type CallbackFunction = fn(u32, u32, &[u8], &mut ComputeState) -> Vec<u8>;

pub struct DiPsProperties {
    video_path: Option<String>,
    frame_callback: Option<Arc<Mutex<CallbackFunction>>>,
}

impl DiPsProperties {
    pub fn new() -> Self {
        Self {
            video_path: None,
            frame_callback: None,
        }
    }

    /// Sets the video path using the builder structure
    pub fn video_path<S>(&mut self, video_path: S) -> &mut Self
    where
        S: AsRef<str>,
    {
        self.video_path = Some(String::from(video_path.as_ref()));

        self
    }

    /// Sets the frame callback function using the builder structure
    pub fn frame_callback(&mut self, frame_callback: CallbackFunction) -> &mut Self {
        self.frame_callback = Some(Arc::new(Mutex::new(frame_callback)));

        self
    }

    pub fn build(&self) -> Self {
        Self {
            video_path: self.video_path.clone(),
            frame_callback: self.frame_callback.clone(),
        }
    }
}

// Custom Error Types
#[derive(Debug)]
pub struct VideoPathNotSpecifiedError;

impl Error for VideoPathNotSpecifiedError {
    fn description(&self) -> &str {
        "Video Path not specified in the DiPs Properties"
    }
}

impl Display for VideoPathNotSpecifiedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Video Path Not Specified")
    }
}

#[derive(Debug)]
pub struct FrameCallbackNotSpecifiedError;

impl Error for FrameCallbackNotSpecifiedError {
    fn description(&self) -> &str {
        "Frame Callback not specified in the DiPs Properties"
    }
}

impl Display for FrameCallbackNotSpecifiedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Frame Callback Not Specified")
    }
}

#[derive(Debug)]
pub struct StreamNotFoundError;

impl Error for StreamNotFoundError {
    fn description(&self) -> &str {
        "Video Stream not found"
    }
}

impl Display for StreamNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Video Stream not found")
    }
}

#[derive(Debug)]
pub struct StreamPipelineError;

impl Error for StreamPipelineError {
    fn description(&self) -> &str {
        "Stream Pipeline error"
    }
}

impl Display for StreamPipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Video Stream Error")
    }
}

fn frame_callback(
    width: u32,
    height: u32,
    frame_data: &[u8],
    compute: &mut ComputeState,
) -> Vec<u8> {
    if !compute.has_initial_frame() {
        compute.add_initial_texture(width, height, frame_data);
    }

    compute.update_input_texture(frame_data);
    compute.dispatch();

    compute.get_pixels()
}

pub fn test_video_get() {
    pretty_env_logger::init();
    initialize_gstreamer();

    let props = DiPsProperties::new()
        .video_path("test_files/diffraction_short.avi")
        .frame_callback(frame_callback)
        .build();

    _ = create_video_frame_decoder_pipeline(&props)
        .and_then(|(pipeline, compute_state)| run_pipeline(pipeline, compute_state));
}
