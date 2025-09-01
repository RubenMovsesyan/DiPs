use std::path::Path;

use anyhow::{Result, anyhow};
use ffmpeg_next::{
    self as ffmpeg, Rational,
    format::{self, context::Output},
    frame, media,
    software::{self, scaling::Context},
};

pub fn modify_video<P, F>(input_file: P, output_file: P, mut intermediary_function: F) -> Result<()>
where
    P: AsRef<Path>,
    F: FnMut(&[u8], &mut VideoEncoder),
{
    ffmpeg::init()?;
    let mut input = format::input(&input_file)?;

    let stream = input
        .streams()
        .best(media::Type::Video)
        .ok_or_else(|| anyhow!("No Video stream found"))?;
    let stream_index = stream.index();

    // Get a decoder
    let context_decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
    let mut decoder = context_decoder.decoder().video()?;

    // Create frame buffers
    let mut decoded_frame = frame::Video::empty();
    let mut rgba_frame = frame::Video::empty();

    let width = decoder.width();
    let height = decoder.height();
    let frame_rate = decoder.frame_rate().unwrap_or_else(|| {
        println!("Could not get frame rate... Using default frame rate");
        Rational(1, 30)
    });

    let mut scaler = software::scaling::Context::get(
        decoder.format(),
        width,
        height,
        ffmpeg::format::Pixel::RGBA,
        width,
        height,
        software::scaling::Flags::BILINEAR,
    )?;

    let mut video_encoder = VideoEncoder::new(output_file, width, height, frame_rate)?;

    let mut recieve_frame = |decoder: &mut ffmpeg::decoder::Video| -> Result<()> {
        // Receive frames
        while decoder.receive_frame(&mut decoded_frame).is_ok() {
            // Convert to RGBA
            scaler.run(&decoded_frame, &mut rgba_frame)?;

            // Extract raw bytes
            let data = rgba_frame.data(0);
            let stride = rgba_frame.stride(0) as usize;
            let mut frame_bytes = Vec::new();

            for y in 0..height as usize {
                let row_start = y * stride;
                let row_end = row_start + (width as usize * 4); // 4 bytes per pixel
                frame_bytes.extend_from_slice(&data[row_start..row_end]);
            }

            intermediary_function(&frame_bytes, &mut video_encoder);
        }

        Ok(())
    };

    // Process packets
    for (stream, packet) in input.packets() {
        if stream.index() == stream_index {
            decoder.send_packet(&packet)?;
            recieve_frame(&mut decoder)?;
        }
    }

    // Send empty packet to flush decoder
    decoder.send_eof().ok();
    recieve_frame(&mut decoder)?;

    video_encoder.finish()
}

pub struct VideoEncoder {
    frame_index: i64,
    rgba_frame: frame::Video,
    yuv_frame: frame::Video,
    encoder: ffmpeg::encoder::Video,
    video_stream_time_base: Rational,
    scaler: Context,
    output: Output,
    frame_rate: Rational,
    pub width: u32,
    pub height: u32,
}

impl VideoEncoder {
    pub fn new<P>(output_path: P, width: u32, height: u32, frame_rate: Rational) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        ffmpeg::init()?;

        let mut output = format::output(&output_path)?;

        let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::H264);
        let mut video_stream = output.add_stream(codec)?;

        let mut encoder = ffmpeg::codec::context::Context::new_with_codec(
            codec.ok_or(ffmpeg::Error::InvalidData)?,
        )
        .encoder()
        .video()?;

        video_stream.set_parameters(&encoder);
        encoder.set_width(width);
        encoder.set_height(height);
        encoder.set_format(ffmpeg::format::Pixel::YUV420P);
        encoder.set_time_base(frame_rate);

        let opened_encoder = encoder.open()?;
        video_stream.set_parameters(&opened_encoder);
        video_stream.set_time_base(frame_rate);

        let video_stream_time_base = video_stream.time_base();

        let scaler = software::scaling::Context::get(
            ffmpeg::format::Pixel::RGBA,
            width,
            height,
            ffmpeg::format::Pixel::YUV420P,
            width,
            height,
            software::scaling::Flags::BILINEAR,
        )?;

        let rgba_frame = frame::Video::new(ffmpeg::format::Pixel::RGBA, width, height);
        let yuv_frame = frame::Video::new(ffmpeg::format::Pixel::YUV420P, width, height);

        output.write_header()?;

        Ok(Self {
            frame_index: 0,
            video_stream_time_base,
            encoder: opened_encoder,
            scaler,
            rgba_frame,
            yuv_frame,
            output,
            frame_rate,
            width,
            height,
        })
    }

    pub fn encode_frame(&mut self, frame_bytes: &[u8]) -> Result<()> {
        let rgba_data = self.rgba_frame.data_mut(0);
        rgba_data[..frame_bytes.len()].copy_from_slice(frame_bytes);

        // Convert RGBA to YUV420P
        self.scaler.run(&self.rgba_frame, &mut self.yuv_frame)?;

        // Set frame properties
        self.yuv_frame.set_pts(Some(self.frame_index));
        self.frame_index += 1;

        self.encoder.send_frame(&self.yuv_frame)?;

        let mut encoded = ffmpeg::Packet::empty();
        while self.encoder.receive_packet(&mut encoded).is_ok() {
            encoded.set_stream(0);
            encoded.rescale_ts(self.frame_rate, self.video_stream_time_base);
            encoded.write_interleaved(&mut self.output)?;
        }

        Ok(())
    }

    pub fn finish(&mut self) -> Result<()> {
        self.encoder.send_eof()?;
        let mut encoded = ffmpeg::Packet::empty();
        while self.encoder.receive_packet(&mut encoded).is_ok() {
            encoded.set_stream(0);
            encoded.rescale_ts(self.frame_rate, self.video_stream_time_base);
            encoded.write_interleaved(&mut self.output)?;
        }
        self.output.write_trailer()?;
        Ok(())
    }
}
