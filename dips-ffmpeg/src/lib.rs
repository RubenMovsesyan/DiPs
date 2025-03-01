mod frame_extractor;
mod gpu;

use std::error::Error;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::Mutex;

use frame_extractor::extract_frames;
use frame_extractor::initialize_frame_extractor;
use gpu::ComputeState;

type CallbackFunction = fn(u32, u32, &[u8], &mut ComputeState) -> Vec<u8>;

pub struct DiPsProperties {
    video_path: Option<String>,
    frame_callback: Option<CallbackFunction>,
    output_path: Option<String>,
}

impl DiPsProperties {
    pub fn new() -> Self {
        Self {
            video_path: None,
            frame_callback: None,
            output_path: None,
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
        self.frame_callback = Some(frame_callback);

        self
    }

    /// Sets the output path using the builder structure
    pub fn output_path<S>(&mut self, output_path: S) -> &mut Self
    where
        S: AsRef<str>,
    {
        self.output_path = Some(String::from(output_path.as_ref()));

        self
    }

    pub fn get_video_path(&self) -> Option<&String> {
        self.video_path.as_ref()
    }

    pub fn get_output_path(&self) -> Option<&String> {
        self.output_path.as_ref()
    }

    pub fn build(&self) -> Self {
        Self {
            video_path: self.video_path.clone(),
            frame_callback: self.frame_callback.clone(),
            output_path: self.output_path.clone(),
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
    compute.add_texture(width, height, frame_data);

    if compute.can_dispatch() {
        compute.dispatch();
        compute.get_pixels()
    } else {
        frame_data.to_vec()
    }
}

pub fn init_frame_extractor() {
    initialize_frame_extractor();
}

pub fn perform_dips(properties: &mut DiPsProperties) {
    properties.frame_callback(frame_callback);
    _ = extract_frames(properties);
}
