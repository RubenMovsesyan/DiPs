use byte_slice_cast::*;
use gstreamer::{
    element_error,
    glib::{self, closure::TryFromClosureReturnValue},
    prelude::{ElementExt, GstObjectExt},
    ClockTime, FlowError, FlowSuccess, ResourceError, State,
};
// Logging
use log::*;

mod frame_extractor;

use frame_extractor::*;

pub fn test_video_get(path: &str) {
    pretty_env_logger::init();
    initialize_gstreamer();
    _ = create_video_frame_decoder_element(path).and_then(
        |pipeline| -> Result<(), Box<dyn std::error::Error>> {
            pipeline.set_state(State::Playing)?;

            let bus = pipeline
                .bus()
                .expect("Pipeline without bus. Shouldn't happen!");

            for msg in bus.iter_timed(ClockTime::NONE) {
                use gstreamer::MessageView;
                // info!("Message: {:#?}", msg);

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
