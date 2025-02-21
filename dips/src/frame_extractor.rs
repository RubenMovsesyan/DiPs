use gstreamer::{element_error, element_warning, prelude::*, CoreError, LibraryError, Structure};
// logging
use log::*;

// std
use std::env;

// gstreamer imports
use gstreamer::{self as gst, Element, FlowError, FlowSuccess, Pipeline};
use gstreamer::{Caps, ElementFactory};
use gstreamer_app::{self, AppSink, AppSinkCallbacks};

// Temp

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

pub fn create_video_frame_decoder_element(
    video_path: &str,
) -> Result<Pipeline, Box<dyn std::error::Error>> {
    // filesrc -> decodebin -> videoconvert -> appsink

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

                // let sink = ElementFactory::make("autovideosink").build()?;

                // let sink = ElementFactory::make("appsink")
                //     .name("Video Frame App Sink")
                //     .property("emit-signals", &true)
                //     .property("sync", &false)
                //     .build()?
                //     .dynamic_cast::<AppSink>();

                let sink = AppSink::builder()
                    .caps(&Caps::builder("video/x-raw").field("format", &"RGB").build())
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
                        .new_sample(|appsink| {
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
