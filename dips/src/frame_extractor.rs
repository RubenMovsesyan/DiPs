// logging
use log::*;

// std
use std::env;
use std::sync::{Arc, RwLock};

// gstreamer imports
use gstreamer::{self as gst, Element, FlowError, FlowSuccess, Pipeline};
use gstreamer::{element_error, element_warning, prelude::*, CoreError, LibraryError};
use gstreamer::{Caps, ElementFactory};
use gstreamer_app::{self, AppSink, AppSinkCallbacks};

use crate::gpu::ComputeState;
use crate::DiPsProperties;
use crate::{FrameCallbackNotSpecifiedError, VideoPathNotSpecifiedError};

pub fn initialize_gstreamer() {
    env::set_var("GST_DEBUG", "3");

    gst::init().unwrap();
    let (gst_version_major, gst_version_minor, gst_version_micro, gst_version_nano) =
        gst::version();

    let nano_str = if gst_version_nano == 1 {
        "(CVS)"
    } else if gst_version_nano == 2 {
        "(Prelease)"
    } else {
        ""
    };

    info!(
        "This program is linked against GStreamer {}.{}.{} {}",
        gst_version_major, gst_version_minor, gst_version_micro, nano_str
    );
}

pub fn create_video_frame_decoder_pipeline(
    properties: &DiPsProperties,
) -> Result<(Pipeline, Arc<RwLock<ComputeState>>), Box<dyn std::error::Error>> {
    // Extracts the video path
    // If the video path is not specified then return an error
    let video_path = match properties.video_path.as_ref() {
        Some(path) => path,
        None => return Err(Box::new(VideoPathNotSpecifiedError)),
    };

    // -------------------- Build the Pipeline --------------------------

    // Create a frame decoding pipeline
    let frame_decoding_pipeline = Pipeline::default();

    // Create source element
    let source = ElementFactory::make("filesrc")
        .name("Video Frame Decoder Source")
        .property("location", video_path)
        .build()?;

    // Create a decodebin element
    let decodebin = ElementFactory::make("decodebin")
        .name("Video Frame Decoder Video Decoder")
        .build()?;

    frame_decoding_pipeline.add_many([&source, &decodebin])?;
    Element::link_many([&source, &decodebin])?;

    let pipeline_weak = frame_decoding_pipeline.downgrade();

    // GPU Compute
    let compute = Arc::new(RwLock::new(
        ComputeState::new().expect("Could not create Compute State"),
    ));
    let compute_closure_clone = compute.clone();

    // Frame Callback cloning
    let frame_callback_closure_clone = match properties.frame_callback.as_ref() {
        Some(callback) => callback.clone(),
        None => {
            return Err(Box::new(FrameCallbackNotSpecifiedError));
        }
    };

    decodebin.connect_pad_added(move |dbin, src_pad| {
        let Some(pipeline) = pipeline_weak.upgrade() else {
            return;
        };

        let is_video = {
            let media_type = src_pad.current_caps().and_then(|caps| {
                caps.structure(0).map(|s| {
                    let name = s.name();
                    name.starts_with("video/")
                })
            });

            match media_type {
                None => {
                    element_warning!(
                        dbin,
                        CoreError::Negotiation,
                        ("Failed to get media type from pad {}", src_pad.name())
                    );

                    return;
                }
                Some(media_type) => media_type,
            }
        };

        // Creating clones to send into sink closure
        let compute_clone = compute_closure_clone.clone();
        let frame_callback_clone = frame_callback_closure_clone.clone();

        let insert_sink = |is_video| -> Result<(), Box<dyn std::error::Error>> {
            if is_video {
                // Create the pipeline to queue each frame, convert it into a readable format, scale it, and sink the data to the app
                let queue = ElementFactory::make("queue")
                    .name("Video Frame Queue")
                    .build()?;
                let convert = ElementFactory::make("videoconvert")
                    .name("Video Frame Converter")
                    .build()?;
                let scale = ElementFactory::make("videoscale")
                    .name("Video Frame Scale")
                    .build()?;

                let sink = AppSink::builder()
                    .caps(
                        &Caps::builder("video/x-raw")
                            .field("format", &"RGBA")
                            .build(),
                    )
                    .sync(false) // This is done so the pipeline doesn't wait for the timestamps of each frame and runs through as quick as possible
                    .build();

                let elements = &[&queue, &convert, &scale, sink.upcast_ref()];
                pipeline.add_many(elements)?;
                Element::link_many(elements)?;

                for e in elements {
                    e.sync_state_with_parent()?
                }

                let sink_pad = queue.static_pad("sink").expect("queue has no sinkpad");
                src_pad.link(&sink_pad)?;

                // Create the callback for the app sink
                sink.set_callbacks(
                    AppSinkCallbacks::builder()
                        .new_sample(move |appsink| {
                            match appsink.pull_sample() {
                                Ok(sample) => {
                                    // Retrieve Frame info (width, height) and data bytes
                                    let (width, height) = if let Some(caps) = sample.caps() {
                                        if let Some(s) = caps.structure(0) {
                                            (
                                                s.get::<i32>("width").unwrap_or(0),
                                                s.get::<i32>("height").unwrap_or(0),
                                            )
                                        } else {
                                            (0, 0)
                                        }
                                    } else {
                                        (0, 0)
                                    };

                                    let buffer = sample.buffer().expect("Failed to get buffer");
                                    let map = buffer.map_readable().expect("Failed to map buffer");

                                    let frame_data = map.as_slice();

                                    info!(
                                        "width: {:#?} height: {:#?}, Data len: {}",
                                        width,
                                        height,
                                        frame_data.len()
                                    );

                                    if let Ok(mut compute) = compute_clone.write() {
                                        if !compute.has_initial_frame() {
                                            compute
                                                .add_initial_texture(width as u32, height as u32);
                                        }

                                        // Here is where the callback is called for each frame
                                        if let Ok(callback) = frame_callback_clone.lock() {
                                            callback(
                                                width as u32,
                                                height as u32,
                                                frame_data,
                                                &mut compute,
                                            );
                                        }
                                    }

                                    Ok(FlowSuccess::Ok)
                                }
                                Err(_) => Err(FlowError::Eos),
                            }
                        })
                        .build(),
                );
            }

            Ok(())
        };

        if let Err(_err) = insert_sink(is_video) {
            element_error!(dbin, LibraryError::Failed, ("Failed to insert sink"));
        }
    });

    Ok((frame_decoding_pipeline, compute.clone()))
}
