//! Pro Wave Android App Entry Point
//! Launches the complete Pro Wave DAW UI on Android via eframe/egui.

#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    log::info!("Launching Pro Wave Full DAW UI on Android...");
    let _ = prowave_app::run_android(app);
}
