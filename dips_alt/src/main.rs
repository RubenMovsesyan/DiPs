use anyhow::*;
use dips_alt::*;

fn main() -> Result<()> {
    pretty_env_logger::init();
    let args = std::env::args().into_iter().collect::<Vec<String>>();
    // _ = run_with_open_cv();
    // run_dips_app()
    run_dips_on_file(&args[1], &args[2])
    // custom_dips_on_files(&args[1], &args[2], &args[3])
}
