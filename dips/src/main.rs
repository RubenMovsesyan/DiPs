use anyhow::*;
use libdips::*;
use opencv::videoio::{self, VideoWriter};

#[derive(Debug)]
pub enum Encoding {
    Uncompressed,
    Huffman,
    H264,
}

impl Encoding {
    fn as_fourcc(&self) -> i32 {
        match self {
            Encoding::Uncompressed => VideoWriter::fourcc('R', 'G', 'B', 'A').expect("Failed"),
            Encoding::Huffman => VideoWriter::fourcc('H', 'F', 'Y', 'U').expect("Failed"),
            Encoding::H264 => VideoWriter::fourcc('H', '2', '6', '4').expect("Failed"),
        }
    }
}

fn main() -> Result<()> {
    pretty_env_logger::init();
    let args = std::env::args().into_iter().collect::<Vec<String>>();

    let mut input_path = String::new();
    let mut output_path = String::new();
    let mut encoding = Encoding::Uncompressed;
    let mut dips_props = DiPsProperties::default();
    let mut refresh_markers: Vec<usize> = Vec::new();

    for arg in args[1..].iter() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(include_str!("help.txt"));
                return Ok(());
            }
            // "--live" => run_dips_app()?,
            _ => {}
        }

        let split = arg.split('=').collect::<Vec<_>>();

        match split[0] {
            "--input" => {
                input_path = split[1].to_string();
            }
            "--output" => {
                output_path = split[1].to_string();
            }
            "--encoding" => {
                encoding = match split[1] {
                    "RGBA" => Encoding::Uncompressed,
                    "HFYU" => Encoding::Huffman,
                    "H264" => Encoding::H264,
                    _ => Encoding::Uncompressed,
                };
            }
            "--filter" => {
                dips_props.set_filter(match split[1] {
                    "sigmoid" => Filter::Sigmoid,
                    "inv_sig" => Filter::InverseSigmoid,
                    _ => return Err(anyhow!("Invalide Filter Type")),
                });
            }
            "--chroma" => {
                dips_props.set_chroma_filter(match split[1] {
                    "r" => ChromaFilter::Red,
                    "g" => ChromaFilter::Green,
                    "b" => ChromaFilter::Blue,
                    _ => return Err(anyhow!("Invalid Chroma Type")),
                });
            }
            "--sig_scalar" => {
                dips_props.set_sigmoid_horizontal_scalar(match split[1].parse::<f32>() {
                    Result::Ok(val) => val,
                    Err(err) => return Err(anyhow!(err)),
                });
            }
            "--win_size" => {
                dips_props.set_window_size(match split[1].parse::<u8>() {
                    Result::Ok(val) => val,
                    Err(err) => return Err(anyhow!(err)),
                });
            }
            "--colorize" => {
                dips_props.set_colorize(match split[1] {
                    "false" => false,
                    _ => true,
                });
            }
            _ => match split[0].parse::<usize>() {
                Result::Ok(parsed) => refresh_markers.push(parsed),
                Err(err) => {
                    return Err(anyhow!(err));
                }
            },
        }
    }

    if input_path.is_empty() {
        return Err(anyhow!("Input file not specified"));
    }

    if output_path.is_empty() {
        return Err(anyhow!("Output file not specified"));
    }

    println!("Running DiPs on file with settings:");
    println!("===================================");
    println!("input path: {}", input_path);
    println!("output path: {}", output_path);
    println!("Encoding: {:#?}", encoding);
    println!("Properties: {:#?}", dips_props);
    println!("Refresh Markers: {:#?}", refresh_markers);
    println!();

    run_dips_on_file(
        input_path,
        output_path,
        encoding,
        dips_props,
        refresh_markers,
    )
}
