use gstreamer_app::AppSink;
use log::*;

use gstreamer::{self as gst, element_error, ClockTime, FlowError, Pipeline, ResourceError, State};
use gstreamer_video::prelude::*;
use std::{env, error::Error};

use crate::StreamPipelineError;

pub fn initialize_thumbnail_extractor() {
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

pub fn run_thumbnail_pipeline(pipeline: Pipeline) -> Result<(), Box<dyn Error>> {
    pipeline.set_state(State::Playing)?;

    let bus = pipeline
        .bus()
        .expect("Pipeline without bus, Shouldn't happen!");

    for msg in bus.iter_timed(ClockTime::from_seconds(5)) {
        use gst::MessageView;

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

pub fn extract_thumbnail_pipeline(
    video_path: &str,
    output_image: &str,
) -> Result<Pipeline, Box<dyn Error>> {
    let output_path = String::from(output_image);

    let pipeline = gst::parse::launch(&format!(
        "filesrc location={video_path} ! decodebin ! videoconvert ! appsink name=sink",
    ))?
    .downcast::<Pipeline>()
    .expect("Expected a gst::Pipeline");

    let appsink = pipeline
        .by_name("sink")
        .expect("Sink element not found")
        .downcast::<AppSink>()
        .expect("Sink element is expected to be an AppSink!");

    appsink.set_property("sync", false);

    appsink.set_caps(Some(
        &gstreamer_video::VideoCapsBuilder::new()
            .format(gstreamer_video::VideoFormat::Rgbx)
            .build(),
    ));

    let mut got_snapshot = false;

    appsink.set_callbacks(
        gstreamer_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                let sample = appsink.pull_sample().map_err(|_| FlowError::Eos)?;
                let buffer = sample.buffer().ok_or_else(|| {
                    element_error!(
                        appsink,
                        ResourceError::Failed,
                        ("Failed to get buffer from appsink")
                    );
                    FlowError::Error
                })?;

                if got_snapshot {
                    return Err(FlowError::Eos);
                }

                got_snapshot = true;

                let caps = sample.caps().expect("Sample without caps");
                let info =
                    gstreamer_video::VideoInfo::from_caps(caps).expect("Failed to parse caps");

                let frame = gstreamer_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
                    .map_err(|_| {
                        element_error!(
                            appsink,
                            ResourceError::Failed,
                            ("Failed to map buffer readable")
                        );

                        FlowError::Error
                    })?;

                info!("Have video frame");

                let display_aspect_ratio = (frame.width() as f64 * info.par().numer() as f64)
                    / (frame.height() as f64 * info.par().numer() as f64);

                let target_height = 240;
                let target_width = target_height as f64 * display_aspect_ratio;

                let img = image::FlatSamples::<&[u8]> {
                    samples: frame.plane_data(0).unwrap(),
                    layout: image::flat::SampleLayout {
                        channels: 3,
                        channel_stride: 1,
                        width: frame.width(),
                        width_stride: 4,
                        height: frame.height(),
                        height_stride: frame.plane_stride()[0] as usize,
                    },
                    color_hint: Some(image::ColorType::Rgb8),
                };

                let scaled_img = image::imageops::thumbnail(
                    &img.as_view::<image::Rgb<u8>>()
                        .expect("couldn't create image view"),
                    target_width as u32,
                    target_height as u32,
                );

                scaled_img.save(&output_path).map_err(|err| {
                    element_error!(
                        appsink,
                        ResourceError::Write,
                        ("Falied to write thumbnail file {}: {}", output_path, err)
                    );

                    FlowError::Error
                })?;

                Err(FlowError::Eos)
            })
            .build(),
    );

    Ok(pipeline)
}
