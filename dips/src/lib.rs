use std::{error::Error, fmt::Display};

use gstreamer::{
    prelude::{ElementExt, GstObjectExt},
    ClockTime, State,
};
// Logging
use log::*;

mod frame_extractor;
mod gpu;

use frame_extractor::*;

pub struct DiPsProperties {
    video_path: Option<String>,
    frame_callback: Option<fn(&[u8])>,
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
    pub fn frame_callback(&mut self, frame_callback: fn(&[u8])) -> &mut Self {
        self.frame_callback = Some(frame_callback);

        self
    }

    pub fn build(&self) -> Self {
        Self {
            video_path: self.video_path.clone(),
            frame_callback: self.frame_callback,
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

pub fn test_video_get() {
    pretty_env_logger::init();
    initialize_gstreamer();

    let props = DiPsProperties::new()
        .video_path("test_files/diffraction.avi")
        .build();

    _ = create_video_frame_decoder_pipeline(&props).and_then(
        |pipeline| -> Result<(), Box<dyn std::error::Error>> {
            pipeline.set_state(State::Playing)?;

            let bus = pipeline
                .bus()
                .expect("Pipeline without bus. Shouldn't happen!");

            for msg in bus.iter_timed(ClockTime::NONE) {
                use gstreamer::MessageView;

                match msg.view() {
                    MessageView::Eos(..) => break,
                    MessageView::Error(err) => {
                        pipeline.set_state(State::Null)?;
                        return Err(todo!());
                    }
                    MessageView::StateChanged(s) => {
                        info!(
                            "State Changed from {:#?}: {:#?} -> {:#?} ({:#?})",
                            s.src().map(|s| s.path_string()),
                            s.old(),
                            s.current(),
                            s.pending()
                        );
                    }
                    _ => (),
                }
            }

            pipeline.set_state(State::Null)?;

            Ok(())
        },
    );
}
