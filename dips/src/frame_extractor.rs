// logging
use log::*;

use std::cell::RefCell;
// std
use std::env;
use std::ops::Deref;
use std::rc::Rc;
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
) -> Result<Pipeline, Box<dyn std::error::Error>> {
    // Create a frame decoding pipeline
    let frame_decoding_pipeline = Pipeline::default();

    // Create source element
    // If the video path is not specified then return an error
    let source = ElementFactory::make("filesrc")
        .name("Video Frame Decoder Source")
        .property(
            "location",
            match properties.video_path.as_ref() {
                Some(path) => path,
                None => return Err(Box::new(VideoPathNotSpecifiedError)),
            },
        )
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

        let insert_sink = |is_video| -> Result<(), Box<dyn std::error::Error>> {
            if is_video {
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
                let compute_clone = compute.clone();

                // FIXME: This should be in the callback but its not working for some reason
                compute_clone.write().unwrap().add_initial_texture(640, 480);

                // Create the callback for the app sink
                sink.set_callbacks(
                    AppSinkCallbacks::builder()
                        .new_sample(move |appsink| {
                            match appsink.pull_sample() {
                                Ok(sample) => {
                                    let buffer = sample.buffer().expect("Failed to get buffer");
                                    let map = buffer.map_readable().expect("Failed to map buffer");

                                    // Retrieve Frame info (width, height)
                                    if let Some(caps) = sample.caps() {
                                        if let Some(s) = caps.structure(0) {
                                            let width = s.get("width").unwrap_or(0);
                                            let height = s.get("height").unwrap_or(0);

                                            let frame_data = map.as_slice();

                                            info!(
                                                "width: {} height: {}, Data len: {}",
                                                width,
                                                height,
                                                frame_data.len()
                                            );

                                            // if !compute_clone
                                            //     .read()
                                            //     .expect("Could Not obtain read")
                                            //     .has_initial_frame()
                                            // {
                                            //     compute_clone
                                            //         .write()
                                            //         .expect("Could Not obtain Write")
                                            //         .add_initial_texture(width, height);
                                            // }

                                            compute_clone
                                                .read()
                                                .expect("Could Not Obtain Read")
                                                .update_input_texture(frame_data);
                                            compute_clone
                                                .read()
                                                .expect("Could Not obtain Read")
                                                .dispatch();

                                            // let mut max_r = frame_data[0];
                                            // let mut max_g = frame_data[1];
                                            // let mut max_b = frame_data[2];

                                            // let mut r_sum: u64 = 0;
                                            // let mut g_sum: u64 = 0;
                                            // let mut b_sum: u64 = 0;

                                            // for index in (0..frame_data.len()).step_by(4) {
                                            //     let (r, g, b) = (
                                            //         frame_data[index],
                                            //         frame_data[index + 1],
                                            //         frame_data[index + 2],
                                            //     );

                                            //     r_sum += r as u64;
                                            //     g_sum += g as u64;
                                            //     b_sum += b as u64;

                                            //     max_r = max_r.max(r);
                                            //     max_g = max_g.max(g);
                                            //     max_b = max_b.max(b);
                                            // }

                                            // r_sum /= (frame_data.len() / 3) as u64;
                                            // g_sum /= (frame_data.len() / 3) as u64;
                                            // b_sum /= (frame_data.len() / 3) as u64;

                                            // info!(
                                            //     "Color Averages:\n    r: {}\n    g: {}\n    b:{}",
                                            //     r_sum, g_sum, b_sum
                                            // );

                                            // info!(
                                            //     "Max r: {} Max g: {} Max b: {}",
                                            //     max_r, max_g, max_b
                                            // );
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

        if let Err(err) = insert_sink(is_video) {
            element_error!(dbin, LibraryError::Failed, ("Failed to insert sink"));
        }
    });

    Ok(frame_decoding_pipeline)
}
