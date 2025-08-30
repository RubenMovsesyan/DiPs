use anyhow::Result;
use libdips::{DiPs, DiPsProperties, Features, GpuController, Limits};
use opencv::{
    core::{AlgorithmHint, VecN},
    imgproc,
    prelude::*,
    videoio::{self, VideoCaptureTrait, VideoCaptureTraitConst},
};
use std::path::Path;

use crate::Encoding;

const FRAME_COUNT: usize = 2;

pub fn run_dips_on_file<P>(
    path: P,
    output: P,
    encoding: Encoding,
    properites: DiPsProperties,
    refresh_markers: Vec<usize>,
) -> Result<()>
where
    P: AsRef<Path>,
{
    let gpu_controller = smol::block_on(GpuController::new(
        Some(Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES),
        Some(Limits {
            max_bind_groups: 5,
            ..Default::default()
        }),
        None,
    ))?;

    let mut overall_frame: usize = 0;
    let mut index: usize = 0;

    let mut file_stream = videoio::VideoCapture::from_file(
        path.as_ref().as_os_str().to_str().unwrap(),
        videoio::CAP_ANY,
    )?;

    let fps = file_stream.get(videoio::CAP_PROP_FPS)?;

    let fourcc = encoding.as_fourcc();
    let mut output_stream = None;

    if !file_stream.is_opened()? {
        panic!("Failed to open file");
    }

    let mut frame = Mat::default();
    let mut compute_state: Option<DiPs> = None;

    loop {
        if !file_stream.read(&mut frame)? {
            break;
        }

        let pts = file_stream.get(videoio::CAP_PROP_PTS)?;
        let dts = file_stream.get(videoio::CAP_PROP_DTS_DELAY)?;

        let width = frame.rows();
        let height = frame.cols();

        if compute_state.is_none() {
            compute_state = Some(DiPs::new(
                FRAME_COUNT,
                width as u32,
                height as u32,
                gpu_controller.clone(),
                properites,
            )?);
        }

        if output_stream.is_none() {
            output_stream = Some(videoio::VideoWriter::new(
                output.as_ref().as_os_str().to_str().unwrap(),
                fourcc,
                fps,
                opencv::core::Size::new(height, width),
                true,
            )?);
        }

        let mut rgba_frame = Mat::default();

        imgproc::cvt_color(
            &frame,
            &mut rgba_frame,
            imgproc::COLOR_BGR2RGBA,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;

        let bytes = rgba_frame.data_bytes()?;

        let new_frame_data = unsafe {
            compute_state.as_mut().unwrap_unchecked().send_frame(
                &bytes,
                match index {
                    FRAME_COUNT => Some(()),
                    _ => None,
                },
                None,
            )
        };

        let new_frame =
            match Mat::new_rows_cols_with_bytes::<VecN<u8, 4>>(width, height, &new_frame_data) {
                Ok(t) => t,
                Err(err) => {
                    println!("Error: {:#?}", err);
                    return Err(anyhow::Error::new(err));
                }
            };

        let mut output_frame = Mat::default();
        imgproc::cvt_color(
            &new_frame,
            &mut output_frame,
            imgproc::COLOR_RGBA2BGR,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;

        if index <= FRAME_COUNT {
            index += 1;
        }

        overall_frame += 1;

        if refresh_markers.contains(&overall_frame) {
            index = 0;
        }

        if let Some(stream) = output_stream.as_mut() {
            print!("\rFrame: {}", overall_frame);
            stream.set(videoio::VIDEOWRITER_PROP_PTS, pts)?;
            stream.set(videoio::VIDEOWRITER_PROP_DTS_DELAY, dts)?;
            stream.write(&output_frame)?;
        }
    }
    println!();

    if let Some(mut writer) = output_stream.take() {
        writer.release()?;
    }

    Ok(())
}
