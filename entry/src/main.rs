slint::include_modules!();

use directories::ProjectDirs;

use log::*;

use native_dialog::FileDialog;
use slint::SharedString;
use std::fs;

use dips::{self, DiPsProperties};

fn get_thumbnail(path: &str) -> slint::Image {
    // Store a thumbnail of the input video
    if let Some(proj_dirs) = ProjectDirs::from("com", "Ruben", "DiPs") {
        let project_data_local_dir = proj_dirs.data_local_dir();

        let output_path = {
            let dir = project_data_local_dir;
            info!("dir: {:#?}", dir);
            fs::create_dir_all(&dir).expect("Could not create filepath");
            let o_path = dir.join("thumbnail.jpeg");
            String::from(o_path.to_str().unwrap())
        };

        info!("Output: {}", output_path);

        dips::init_thumbnail_extractor();
        dips::extract_thumbnail(path, &output_path);

        let source_thumbnail = image::open(&output_path)
            .expect("Could not open thumbnail")
            .into_rgba8();

        slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(
            source_thumbnail.as_raw(),
            source_thumbnail.width(),
            source_thumbnail.height(),
        ))
    } else {
        panic!("Don't know what to do here")
    }
}

fn get_input_path() -> SharedString {
    let path = FileDialog::new()
        .add_filter("Video Files", &["avi", "mov", "mp4"])
        .show_open_single_file()
        .unwrap();

    match path {
        Some(path) => path.to_str().unwrap().into(),
        None => "".into(),
    }
}

fn main() -> Result<(), slint::PlatformError> {
    pretty_env_logger::init();
    let main_window = MainWindow::new()?;

    main_window.on_find_input_path(move || get_input_path());
    main_window.on_get_thumbnail(move |path| get_thumbnail(&path.to_string()));
    main_window.on_run_dips(move |path| {
        let output_path = FileDialog::new().show_save_single_file().unwrap();

        let output_path = match output_path {
            Some(o_path) => String::from(o_path.to_str().unwrap()),
            None => String::from(""),
        };

        dips::init_frame_extractor();
        let mut dips_properties = DiPsProperties::new()
            .video_path(path.as_str())
            .output_path(output_path)
            .build();

        dips::perform_dips(&mut dips_properties);
    });

    main_window.run()
}

// fn main() {
//     pretty_env_logger::init();
//     dips::init_thumbnail_extractor();

//     dips::extract_thumbnail(
//         "test_files/diffraction_short_new.avi",
//         "test_files/output.jpeg",
//     );

//     dips::init_frame_extractor();

//     let mut dips_properties = DiPsProperties::new()
//         .video_path("test_files/diffraction_short_new.avi")
//         .output_path("test_files/output_diff.avi")
//         .build();

//     dips::perform_dips(&mut dips_properties);
// }
