use std::path::Path;

// use ffmpeg_next as ffmpeg;
use log::*;

use video_rs::decode::Decoder;
// use ffmpeg::format::{input, Pixel};
// use ffmpeg::media::Type;
// use ffmpeg::software::scaling::{context::Context, flag::Flags};
// use ffmpeg::util::frame::video::Video;

use crate::gpu::ComputeState;
use crate::{DiPsProperties, FrameCallbackNotSpecifiedError, VideoPathNotSpecifiedError};

pub fn initialize_frame_extractor() {
    // ffmpeg::init().unwrap();
    video_rs::init().unwrap();
}

pub fn extract_frames(properties: &DiPsProperties) -> Result<(), Box<dyn std::error::Error>> {
    let video_path = match properties.get_video_path() {
        Some(path) => path,
        None => return Err(Box::new(VideoPathNotSpecifiedError)),
    };

    let output_path = match properties.get_output_path() {
        Some(path) => path,
        None => return Err(Box::new(VideoPathNotSpecifiedError)),
    };

    let mut decoder = Decoder::new(Path::new(video_path)).expect("Failed to create decoder");

    for frame in decoder.decode_iter() {
        match frame {
            Ok((_, frame)) => {
                let rgb = frame.slice(ndarray::s![0, 0, ..]).to_slice().unwrap();
                info!("pixel at 0, 0: {}, {}, {}", rgb[0], rgb[1], rgb[2],);
            }
            Err(err) => {
                error!("{:#?}", err);
                break;
            }
        }
        // if let Ok((_, frame)) = frame {
        //     let rgb = frame.slice(ndarray::s![0, 0, ..]).to_slice().unwrap();
        //     info!("pixel at 0, 0: {}, {}, {}", rgb[0], rgb[1], rgb[2],);
        // } else {
        //     break;
        // }
    }

    Ok(())
}

// pub fn extract_frames(properties: &DiPsProperties) -> Result<(), Box<dyn std::error::Error>> {
//     let video_path = match properties.get_video_path() {
//         Some(path) => path,
//         None => return Err(Box::new(VideoPathNotSpecifiedError)),
//     };

//     let output_path = match properties.get_output_path() {
//         Some(path) => path,
//         None => return Err(Box::new(VideoPathNotSpecifiedError)),
//     };

//     if let Ok(mut ictx) = input(video_path) {
//         let input = ictx
//             .streams()
//             .best(Type::Video)
//             .ok_or(ffmpeg::Error::StreamNotFound)?;

//         let video_stream_index = input.index();

//         let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
//         // let mut decoder = input.codec().decoder().video()?;
//         let mut decoder = context_decoder.decoder().video()?;

//         let mut scaler = Context::get(
//             decoder.format(),
//             decoder.width(),
//             decoder.height(),
//             Pixel::RGB24,
//             decoder.width(),
//             decoder.height(),
//             Flags::BILINEAR,
//         )?;

//         let mut frame_index = 0;

//         let mut compute = ComputeState::new().expect("Could not create compute state");

//         let frame_callback_closure = match properties.frame_callback.as_ref() {
//             Some(callback) => callback,
//             None => {
//                 return Err(Box::new(FrameCallbackNotSpecifiedError));
//             }
//         };

//         let mut recieve_and_process_decoded_frames =
//             |decoder: &mut ffmpeg::decoder::Video| -> Result<(), ffmpeg::Error> {
//                 let mut decoded = Video::empty();
//                 while decoder.receive_frame(&mut decoded).is_ok() {
//                     for i in 0..decoded.planes() {
//                         let width = decoded.width() as usize;
//                         let height = decoded.height() as usize;
//                         let stride = decoded.stride(i) as isize;

//                         if decoded.is_top_first() {
//                         }

//                         // let callback_data = frame_callback_closure(
//                         //     decoded.width(),
//                         //     decoded.height(),
//                         //     &buffer,
//                         //     &mut compute,
//                         // );

//                         // info!("new data: {}", callback_data.len());
//                     }

//                     // scaler = Context::get(
//                     //     decoded.format(),
//                     //     decoded.width(),
//                     //     decoded.height(),
//                     //     Pixel::RGB24,
//                     //     decoded.width(),
//                     //     decoded.height(),
//                     //     Flags::BILINEAR,
//                     // )?;

//                     // let mut rgb_frame = Video::empty();
//                     // match scaler.run(&decoded, &mut rgb_frame) {
//                     //     Ok(_) => {
//                     //         warn!("Ran well");
//                     //     }
//                     //     Err(err) => {
//                     //         error!("{:#?}", err);
//                     //     }
//                     // };

//                     // for i in 0..decoded.planes() {
//                     //     info!(
//                     //         "line size: {} format: {:#?}",
//                     //         decoded.stride(i),
//                     //         decoded.format()
//                     //     );
//                     // }
//                     // info!("{:#?}", decoded.plane::<(u8, u8, u8, u8)>(0));

//                     // for i in 0..decoded.planes() {
//                     // Unsafe access to avoid integer underflow when calling data function
//                     // don't need to check if the index is >= planes() because we are in a
//                     // for loop
//                     // let data = unsafe {
//                     //     match decoded.stride(i) as isize {
//                     //         size if size < 0 => {
//                     //             let new_size = (size * -1) as usize;
//                     //             let ptr = *decoded.as_ptr().offset(size);
//                     //             slice::from_raw_parts(
//                     //                 ptr.data[i],
//                     //                 new_size * decoded.plane_height(i) as usize,
//                     //             )
//                     //         }
//                     //         size => slice::from_raw_parts(
//                     //             (*decoded.as_ptr()).data[i],
//                     //             size as usize * decoded.plane_height(i) as usize,
//                     //         ),
//                     //     }

//                     //     // slice::from_raw_parts(
//                     //     //     (*decoded.as_ptr()).data[i],
//                     //     //     match decoded.stride(i) as isize {
//                     //     //         size if size < 0 => (size * -1) as usize,
//                     //     //         size => size as usize,
//                     //     //     } * decoded.plane_height(i) as usize,
//                     //     // )
//                     // };

//                     // info!("data: {}", data.len());

//                     // let callback_data = frame_callback_closure(
//                     //     decoded.width(),
//                     //     decoded.height(),
//                     //     data,
//                     //     &mut compute,
//                     // );

//                     // info!("new_data: {}", callback_data.len());
//                     // }

//                     // let line_size = decoded.stride(i);
//                     // let stride = decoded.stride(i) as i32;
//                     // let plane_height = decoded.plane_height(i);

//                     // info!(
//                     //     "Frame: {} stride: {} Plane height: {}, format: {:#?}",
//                     //     frame_index,
//                     //     stride,
//                     //     plane_height,
//                     //     decoded.format(),
//                     // );
//                     // info!(
//                     //     "Frame: {}, DataLen: {}",
//                     //     frame_index,
//                     //     data.len(),
//                     //     // line_size
//                     // );
//                     // }

//                     // let callback_data = frame_callback_closure(
//                     //     decoded.width(),
//                     //     decoded.height(),
//                     //     decoded.data(),
//                     // )

//                     frame_index += 1;
//                 }

//                 Ok(())
//             };

//         for (stream, packet) in ictx.packets() {
//             if stream.index() == video_stream_index {
//                 decoder.send_packet(&packet)?;
//                 recieve_and_process_decoded_frames(&mut decoder)?;
//             }
//         }
//         decoder.send_eof()?;
//         recieve_and_process_decoded_frames(&mut decoder)?;
//     }

//     Ok(())
// }
