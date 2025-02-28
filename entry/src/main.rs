slint::include_modules!();
use native_dialog::FileDialog;
use slint::SharedString;

use dips::{self, DiPsProperties};

fn get_input_path() -> SharedString {
    let path = FileDialog::new().show_open_single_file().unwrap();

    match path {
        Some(path) => path.to_str().unwrap().into(),
        None => "".into(),
    }
}

fn main() -> Result<(), slint::PlatformError> {
    pretty_env_logger::init();
    let main_window = MainWindow::new()?;

    main_window.on_find_input_path(move || get_input_path());
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
