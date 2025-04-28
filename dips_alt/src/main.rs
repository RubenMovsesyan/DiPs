use anyhow::*;
use dips_alt::*;

fn main() -> Result<()> {
    pretty_env_logger::init();
    let args = std::env::args().into_iter().collect::<Vec<String>>();

    match args[1].as_str() {
        "-f" => run_dips_on_file(
            &args[2],
            &args[3],
            match args[4].as_str() {
                "RGBA" => Encoding::Uncompressed,
                "HFYU" => Encoding::Huffman,
                _ => Encoding::Uncompressed,
            },
            args[5..]
                .iter()
                .map(|v| v.parse().expect(&format!("Failed to parse: {}", v)))
                .collect(),
        ),
        "-c" => run_dips_app(),
        "-d" => custom_dips_on_files(&args[2], &args[3], &args[4]),
        _ => {
            println!("Choose better option");
            Err(anyhow!("Failed"))
        }
    }

    // _ = run_with_open_cv();
    // run_dips_app()
    // run_dips_on_file(&args[1], &args[2])
    // custom_dips_on_files(&args[1], &args[2], &args[3])
}
