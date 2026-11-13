#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    #[cfg(not(target_os = "android"))]
    {
        prowave_app::run_desktop()
    }
    #[cfg(target_os = "android")]
    {
        Ok(())
    }
}
