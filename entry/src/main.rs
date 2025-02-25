use dips::{self, DiPsProperties};

fn main() {
    pretty_env_logger::init();
    dips::init_thumbnail_extractor();

    dips::extract_thumbnail(
        "test_files/diffraction_short_new.avi",
        "test_files/output.jpeg",
    );

    dips::init_frame_extractor();

    let mut dips_properties = DiPsProperties::new()
        .video_path("test_files/diffraction_short_new.avi")
        .output_path("test_files/output_diff.avi")
        .build();

    dips::perform_dips(&mut dips_properties);
}
