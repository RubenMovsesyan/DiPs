use dips::{self, DiPsProperties};

fn main() {
    // let input_file = env::args().nth(1).expect("Cannot Open File");

    // if !Path::new(&input_file).exists() {
    //     panic!("File path doesn't exist");
    // }

    // test_video_get();
    dips::init();

    let mut dips_properties = DiPsProperties::new()
        .video_path("test_files/diffraction_short_new.avi")
        .output_path("test_files/output_diff.avi")
        .build();

    dips::perform_dips(&mut dips_properties);
}
