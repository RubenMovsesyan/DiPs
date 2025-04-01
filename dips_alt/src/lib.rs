use anyhow::{Result, anyhow};
use dips_compute::DiPsCompute;
use opencv::{
    core::{AlgorithmHint, VecN},
    highgui, imgproc,
    prelude::*,
    videoio::{self, VideoCaptureTraitConst},
};

mod dips_compute;
mod utils;

const FRAME_COUNT: usize = 2;
pub fn run_with_open_cv() -> Result<()> {
    highgui::named_window("window", highgui::WINDOW_NORMAL)?;

    // This is the main camera on the device, change index to access other
    // device cameras
    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;

    if !cam.is_opened()? {
        panic!("Could not open camera");
    }

    let mut dips: Option<DiPsCompute> = None;
    let mut frame = Mat::default();
    let mut index: usize = 0;

    loop {
        cam.read(&mut frame)?;

        let width = frame.rows();
        let height = frame.cols();

        if dips.is_none() {
            dips = Some(DiPsCompute::new(FRAME_COUNT, width as u32, height as u32)?);
            println!("w: {}, h: {}", width, height);
        }

        // Convert to rgba to be used in compute shader
        let mut rgba_frame = Mat::default();

        match imgproc::cvt_color(
            &frame,
            &mut rgba_frame,
            imgproc::COLOR_BGR2RGBA,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        ) {
            Ok(t) => t,
            Err(err) => println!("Error: {:#?}", err),
        }

        let bytes = rgba_frame.data_bytes()?;

        let new_frame_data = unsafe {
            dips.as_mut().unwrap_unchecked().send_frame(
                &bytes,
                match index {
                    FRAME_COUNT => Some(()),
                    _ => None,
                },
            )
        };

        if index < FRAME_COUNT + 1 {
            index += 1;
        }

        let new_frame =
            match Mat::new_rows_cols_with_bytes::<VecN<u8, 4>>(width, height, &new_frame_data) {
                Ok(t) => t,
                Err(err) => return Err(anyhow!(err)),
            };

        let mut output_frame = Mat::default();
        imgproc::cvt_color(
            &new_frame,
            &mut output_frame,
            imgproc::COLOR_RGBA2BGR,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;

        match highgui::imshow("window", &output_frame) {
            Ok(t) => t,
            Err(err) => println!("Error: {:#?}", err),
        }

        let key = highgui::wait_key(1)?;

        // If pressing q then quit
        if key == 'q' as i32 {
            break;
        } else if key == 's' as i32 {
            index = 0;
        }
    }

    Ok(())
}
