// logging
use log::*;

// std
use std::sync::{Arc, Mutex, RwLock};

// gstreamer imports
use gstreamer::{
    self as gst, Buffer, ClockTime, Element, FlowError, FlowSuccess, Format, Pipeline, State,
};
use gstreamer::{Caps, ElementFactory};
use gstreamer::{CoreError, LibraryError, element_error, element_warning, prelude::*};
use gstreamer_app::{self, AppSink, AppSinkCallbacks, AppSrc};

use crate::gpu::ComputeState;
use crate::{DiPsProperties, StreamPipelineError};
use crate::{FrameCallbackNotSpecifiedError, VideoPathNotSpecifiedError};

pub fn initialize_frame_extractor() {
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
    // Extracts the video path
    // If the video path is not specified then return an error
    let video_path = match properties.get_video_path() {
        Some(path) => path,
        None => return Err(Box::new(VideoPathNotSpecifiedError)),
    };

    let output_path = match properties.get_output_path() {
        Some(path) => path,
        None => return Err(Box::new(VideoPathNotSpecifiedError)),
    }
    .clone();

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

                // Sink to send frame data to app
                let sink = AppSink::builder()
                    .caps(
                        &Caps::builder("video/x-raw")
                            .field("format", &"RGBA")
                            .build(),
                    )
                    .sync(false) // This is done so the pipeline doesn't wait for the timestamps of each frame and runs through as quick as possible
                    .build();

                // Source to send data from app back into the pipeline
                let src = AppSrc::builder().format(Format::Time).build();

                // Convert to raw video format
                let videoconvert = ElementFactory::make("videoconvert")
                    .name("Video Frame to raw format")
                    .build()?;

                // Mux raw frames into AVI container
                let muxer = ElementFactory::make("avimux")
                    .name("Video Frame AviMuxer")
                    .build()?;

                // filesink to write the video
                let filesink = ElementFactory::make("filesink")
                    .name("Video Frame output file")
                    .property("location", output_path.clone()) // HACK: fix this to be in props
                    .build()?;

                // Pipeline description
                let elements = &[
                    &queue,
                    &convert,
                    &scale,
                    sink.upcast_ref(),
                    src.upcast_ref(),
                    &videoconvert,
                    &muxer,
                    &filesink,
                ];
                pipeline.add_many(elements)?;

                Element::link_many(&[&queue, &convert, &scale, sink.upcast_ref()])?;
                Element::link_many(&[src.upcast_ref(), &videoconvert, &muxer, &filesink])?;

                for e in elements {
                    e.sync_state_with_parent()?
                }

                let sink_pad = queue.static_pad("sink").expect("queue has no sinkpad");
                src_pad.link(&sink_pad)?;

                // Shared appsrc for appsink samples to use
                let app_src_shared = Arc::new(Mutex::new(src));
                let app_src_clone = app_src_shared.clone();
                let eos_app_src_clone = app_src_shared.clone();

                // Create the callback for the app sink
                sink.set_callbacks(
                    AppSinkCallbacks::builder()
                        // This is needed to pass on the eos signal from the filesrc
                        .eos(move |_appsink| {
                            if let Ok(appsrc) = eos_app_src_clone.lock() {
                                appsrc.end_of_stream().expect("Faile to send EOS");
                            }
                        })
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
                                    let pts = buffer.pts();
                                    let duration = buffer.duration();

                                    info!("pts: {:#?}", pts);

                                    if let Ok(mut compute) = compute_clone.write() {
                                        // Here is where the callback is called for each frame
                                        if let Ok(callback) = frame_callback_clone.lock() {
                                            let callback_data = callback(
                                                width as u32,
                                                height as u32,
                                                frame_data,
                                                &mut compute,
                                            );

                                            let mut new_buffer = Buffer::from_slice(callback_data);
                                            // Set the PTS and duration of the new buffer
                                            // INFO: This might not be needed
                                            new_buffer.make_mut().set_pts(pts);
                                            new_buffer.make_mut().set_duration(duration);

                                            if let Ok(appsrc) = app_src_clone.lock() {
                                                // Set the caps of the appsrc to the same as the sample
                                                if let Some(caps) = sample.caps() {
                                                    appsrc.set_caps(Some(&caps.copy()));
                                                }

                                                match appsrc.push_buffer(new_buffer) {
                                                    Ok(_) => {
                                                        info!("Successfully pushed to appsrc")
                                                    }
                                                    Err(err) => {
                                                        error!("Error Pushing buffer: {:#?}", err);
                                                        return Err(FlowError::Error);
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    Ok(FlowSuccess::Ok)
                                }
                                Err(_) => {
                                    if let Ok(appsrc) = app_src_clone.lock() {
                                        appsrc.end_of_stream().expect("Failed to send EOS");
                                    }
                                    Err(FlowError::Eos)
                                }
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

    Ok(frame_decoding_pipeline)
}

pub fn run_pipeline(pipeline: Pipeline) -> Result<(), Box<dyn std::error::Error>> {
    pipeline.set_state(State::Playing)?;

    let bus = pipeline
        .bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    for msg in bus.iter_timed(ClockTime::NONE) {
        use gstreamer::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(_err) => {
                pipeline.set_state(State::Null)?;
                return Err(Box::new(StreamPipelineError));
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
}
