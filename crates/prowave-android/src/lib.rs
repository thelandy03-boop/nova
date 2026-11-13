use eframe::egui;

#[derive(Default)]
struct ProWaveAndroidApp {
    playing: bool,
}

impl eframe::App for ProWaveAndroidApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.heading("Pro Wave Studio (Android)");
            ui.label("Logic Pro & SoundCraft hybrid DAW in pure Rust");
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            let btn_text = if self.playing { "⏸  Pausar Reproducción" } else { "▶  Iniciar Reproducción" };
            if ui.button(egui::RichText::new(btn_text).size(20.0)).clicked() {
                self.playing = !self.playing;
            }

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(10.0);
            ui.label("Motor de Audio en Tiempo Real: AAudio / Oboe (Baja latencia)");
            ui.label("Renderizado Gráfico: WGPU / Vulkan móvil");
        });
    }
}

#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let mut options = eframe::NativeOptions::default();
    options.android_app = Some(app);

    let _ = eframe::run_native(
        "Pro Wave Studio",
        options,
        Box::new(|_cc| Ok(Box::new(ProWaveAndroidApp::default()))),
    );
}
