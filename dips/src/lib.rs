use std::{
    error::Error,
    fmt::Display,
    sync::{Arc, Mutex},
};

use gpu::ComputeState;
// Logging
#[allow(unused_imports)]
use log::*;

mod frame_extractor;
mod gpu;
mod thumbnail_extractor;
mod utils;

use frame_extractor::*;
use thumbnail_extractor::{
    extract_thumbnail_pipeline, initialize_thumbnail_extractor, run_thumbnail_pipeline,
};

// Type alias for the callback function
type CallbackFunction = fn(u32, u32, &[u8], &mut ComputeState) -> Vec<u8>;

#[derive(Copy, Clone, Debug)]
pub enum DiPsFilter {
    Unfiltered,
    Sigmoid,
    InverseSigmoid,
}

impl Into<f64> for DiPsFilter {
    fn into(self) -> f64 {
        use DiPsFilter::*;
        match self {
            Unfiltered => 255.0,
            Sigmoid => 0.0,
            InverseSigmoid => 1.0,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ChromaFilter {
    None,
    Red,
    Green,
    Blue,
}

impl Into<f64> for ChromaFilter {
    fn into(self) -> f64 {
        use ChromaFilter::*;
        match self {
            None => 0.0,
            Red => 1.0,
            Green => 2.0,
            Blue => 3.0,
        }
    }
}

pub struct DiPsProperties {
    video_path: Option<String>,
    frame_callback: Option<Arc<Mutex<CallbackFunction>>>,
    output_path: Option<String>,
    pub colorize: bool,
    pub spatial_window_size: i32,
    pub sensitivity: f32,
    pub filter_type: DiPsFilter,
    pub chroma_filter: ChromaFilter,
}

impl DiPsProperties {
    pub fn new() -> Self {
        Self {
            video_path: None,
            frame_callback: None,
            output_path: None,
            colorize: false,
            spatial_window_size: 1,
            sensitivity: 5.0,
            filter_type: DiPsFilter::Unfiltered,
            chroma_filter: ChromaFilter::None,
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

    /// Sets the output path using the builder structure
    pub fn output_path<S>(&mut self, output_path: S) -> &mut Self
    where
        S: AsRef<str>,
    {
        self.output_path = Some(String::from(output_path.as_ref()));

        self
    }

    /// Sets the colorize parameter of DiPs
    pub fn colorize(&mut self, colorize: bool) -> &mut Self {
        self.colorize = colorize;

        self
    }

    /// Sets the spatial window size parameter of DiPs
    pub fn spatial_window_size(&mut self, spatial_window_size: i32) -> &mut Self {
        self.spatial_window_size = spatial_window_size;

        self
    }

    /// Sets the sensitivity parameter of DiPs
    pub fn sensitivity(&mut self, sensitivity: f32) -> &mut Self {
        self.sensitivity = sensitivity;

        self
    }

    /// Sets the filter type parameter of DiPs
    pub fn filter_type(&mut self, filter_type: DiPsFilter) -> &mut Self {
        self.filter_type = filter_type;

        self
    }

    /// Sets the chroma filter parameter of DiPs
    pub fn chroma_filter(&mut self, chroma_filter: ChromaFilter) -> &mut Self {
        self.chroma_filter = chroma_filter;

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
            colorize: self.colorize.clone(),
            spatial_window_size: self.spatial_window_size.clone(),
            sensitivity: self.sensitivity.clone(),
            filter_type: self.filter_type.clone(),
            chroma_filter: self.chroma_filter.clone(),
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

    if let Some(new_frame) = compute.dispatch() {
        new_frame
    } else {
        frame_data.to_vec()
    }
}

pub fn init_frame_extractor() {
    initialize_frame_extractor();
}

pub async fn perform_dips(mut properties: DiPsProperties) {
    properties.frame_callback(frame_callback);

    _ = create_video_frame_decoder_pipeline(&properties)
        .and_then(|pipeline| run_pipeline(pipeline));
}

pub fn init_thumbnail_extractor() {
    initialize_thumbnail_extractor();
}

pub fn extract_thumbnail(input_path: &str, output_path: &str) {
    _ = extract_thumbnail_pipeline(input_path, output_path)
        .and_then(|pipeline| run_thumbnail_pipeline(pipeline));
}
