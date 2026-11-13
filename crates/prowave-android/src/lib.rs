use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Vec2};
use prowave_engine::{Command, Engine, Song, TrackMix};
use std::collections::HashSet;

struct ProWaveAndroidApp {
    engine: Option<Engine>,
    error_msg: Option<String>,
    playing: bool,
    metronome: bool,
    test_tone: bool,
    master_vol: f32,
    bpm: f64,
    octave: i8,
    active_keys: HashSet<u8>,
    selected_track: u16,
    tracks_mute: [bool; 4],
    tracks_solo: [bool; 4],
}

impl Default for ProWaveAndroidApp {
    fn default() -> Self {
        let (engine, error_msg) = match Engine::start() {
            Ok(mut e) => {
                // Configurar canción inicial con 4 pistas estéreo y master fader a 1.0
                let tracks = vec![TrackMix::default(); 4];
                e.send(Command::SetSong(Box::new(Song::new(Vec::new(), tracks))));
                e.send(Command::SetMasterGain { linear: 1.0 });
                (Some(e), None)
            }
            Err(err) => (None, Some(format!("Error de audio: {err}"))),
        };

        Self {
            engine,
            error_msg,
            playing: false,
            metronome: false,
            test_tone: false,
            master_vol: 0.85,
            bpm: 120.0,
            octave: 4,
            active_keys: HashSet::new(),
            selected_track: 0,
            tracks_mute: [false; 4],
            tracks_solo: [false; 4],
        }
    }
}

impl ProWaveAndroidApp {
    fn send_cmd(&mut self, cmd: Command) {
        if let Some(engine) = self.engine.as_mut() {
            engine.send(cmd);
        }
    }

    fn note_on(&mut self, pitch: u8, vel: u8) {
        if self.active_keys.insert(pitch) {
            self.send_cmd(Command::NoteOn {
                pitch,
                vel,
                track: self.selected_track,
            });
        }
    }

    fn note_off(&mut self, pitch: u8) {
        if self.active_keys.remove(&pitch) {
            self.send_cmd(Command::NoteOff { pitch });
        }
    }
}

impl eframe::App for ProWaveAndroidApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Estilo general oscuro tipo Logic Pro
        ui.style_mut().spacing.item_spacing = Vec2::new(6.0, 6.0);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(26, 27, 30)))
            .show(ui, |ui| {
                // 1. Barra Superior / Título y Estado
                ui.horizontal(|ui| {
                    ui.heading(
                        egui::RichText::new("PRO WAVE STUDIO")
                            .color(Color32::from_rgb(230, 230, 235))
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("Logic Pro · Rust Mobile")
                            .size(11.0)
                            .color(Color32::from_rgb(140, 145, 155)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(engine) = &self.engine {
                            let status = format!("AAudio {}Hz", engine.sample_rate());
                            ui.label(
                                egui::RichText::new(status)
                                    .color(Color32::from_rgb(80, 200, 120))
                                    .size(11.0),
                            );
                        } else if let Some(err) = &self.error_msg {
                            ui.label(
                                egui::RichText::new(err)
                                    .color(Color32::from_rgb(240, 90, 80))
                                    .size(11.0),
                            );
                        }
                    });
                });

                ui.separator();

                // 2. Barra de Transporte (Play, Stop, BPM, Metrónomo, Tono de Prueba)
                ui.horizontal(|ui| {
                    let play_color = if self.playing {
                        Color32::from_rgb(45, 180, 80)
                    } else {
                        Color32::from_rgb(40, 42, 48)
                    };

                    let play_btn = egui::Button::new(
                        egui::RichText::new(if self.playing { "⏸ PAUSA" } else { "▶ PLAY" })
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(play_color)
                    .corner_radius(CornerRadius::same(6));

                    if ui.add_sized([100.0, 36.0], play_btn).clicked() {
                        self.playing = !self.playing;
                        if self.playing {
                            self.send_cmd(Command::Play);
                        } else {
                            self.send_cmd(Command::Pause);
                        }
                    }

                    let stop_btn = egui::Button::new(
                        egui::RichText::new("⏹ STOP")
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(Color32::from_rgb(40, 42, 48))
                    .corner_radius(CornerRadius::same(6));

                    if ui.add_sized([80.0, 36.0], stop_btn).clicked() {
                        self.playing = false;
                        self.send_cmd(Command::Stop);
                    }

                    ui.separator();

                    // Control de Tempo
                    ui.label(egui::RichText::new("BPM:").size(13.0));
                    if ui.button("-").clicked() && self.bpm > 40.0 {
                        self.bpm -= 1.0;
                    }
                    ui.label(
                        egui::RichText::new(format!("{:.0}", self.bpm))
                            .strong()
                            .size(14.0)
                            .color(Color32::from_rgb(100, 180, 255)),
                    );
                    if ui.button("+").clicked() && self.bpm < 240.0 {
                        self.bpm += 1.0;
                    }

                    ui.separator();

                    // Metrónomo
                    let metro_text = if self.metronome { "🔔 Metro ON" } else { "🔕 Metro" };
                    if ui
                        .selectable_label(self.metronome, metro_text)
                        .clicked()
                    {
                        self.metronome = !self.metronome;
                        self.send_cmd(Command::SetMetronome { on: self.metronome });
                    }

                    // Tono de prueba 440 Hz
                    let tone_text = if self.test_tone { "🔊 Test 440Hz ON" } else { "🔈 Test Tone" };
                    if ui.selectable_label(self.test_tone, tone_text).clicked() {
                        self.test_tone = !self.test_tone;
                        let (hz, gain) = if self.test_tone { (440.0, 0.15) } else { (0.0, 0.0) };
                        self.send_cmd(Command::SetTestTone { hz, gain });
                    }

                    ui.separator();

                    // Volumen maestro
                    ui.label("Vol:");
                    if ui
                        .add(egui::Slider::new(&mut self.master_vol, 0.0..=1.0).show_value(false))
                        .changed()
                    {
                        self.send_cmd(Command::SetMasterGain {
                            linear: self.master_vol,
                        });
                    }
                });

                ui.separator();

                // 3. Pistas y Mezclador Rápido (4 Pistas principales)
                ui.horizontal(|ui| {
                    let track_names = ["1 · Synth Lead", "2 · Bass", "3 · Drums", "4 · Pad"];
                    for (idx, name) in track_names.iter().enumerate() {
                        let selected = self.selected_track == idx as u16;
                        let track_bg = if selected {
                            Color32::from_rgb(45, 65, 100)
                        } else {
                            Color32::from_rgb(33, 35, 40)
                        };

                        egui::Frame::NONE
                            .fill(track_bg)
                            .stroke(Stroke::new(1.0, Color32::from_rgb(50, 53, 62)))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                if ui.selectable_label(selected, *name).clicked() {
                                    self.selected_track = idx as u16;
                                }
                                ui.horizontal(|ui| {
                                    let m = &mut self.tracks_mute[idx];
                                    if ui.selectable_label(*m, "M").clicked() {
                                        *m = !*m;
                                    }
                                    let s = &mut self.tracks_solo[idx];
                                    if ui.selectable_label(*s, "S").clicked() {
                                        *s = !*s;
                                    }
                                });
                            });
                    }
                });

                ui.separator();

                // 4. Drum Pads (8 Pads de percusión estilo Drum Machine Designer)
                ui.label(
                    egui::RichText::new("DRUM MACHINE DESIGNER (Pads Táctiles)")
                        .size(12.0)
                        .color(Color32::from_rgb(160, 165, 175))
                        .strong(),
                );

                let pad_defs = [
                    ("Kick", 36, Color32::from_rgb(220, 60, 60)),
                    ("Snare", 38, Color32::from_rgb(230, 140, 40)),
                    ("Hi-Hat Cl", 42, Color32::from_rgb(60, 160, 220)),
                    ("Hi-Hat Op", 46, Color32::from_rgb(70, 180, 230)),
                    ("Low Tom", 41, Color32::from_rgb(180, 70, 200)),
                    ("Mid Tom", 47, Color32::from_rgb(160, 80, 210)),
                    ("Clap", 39, Color32::from_rgb(240, 200, 50)),
                    ("Crash", 49, Color32::from_rgb(60, 210, 160)),
                ];

                ui.horizontal(|ui| {
                    for (label, note, color) in pad_defs {
                        let is_down = self.active_keys.contains(&note);
                        let bg = if is_down {
                            Color32::WHITE
                        } else {
                            color
                        };

                        let btn = egui::Button::new(
                            egui::RichText::new(label)
                                .color(if is_down { Color32::BLACK } else { Color32::WHITE })
                                .size(11.0)
                                .strong(),
                        )
                        .fill(bg)
                        .corner_radius(CornerRadius::same(8));

                        let resp = ui.add_sized([72.0, 55.0], btn);
                        if resp.drag_started() || resp.clicked() {
                            self.note_on(note, 110);
                        }
                        if resp.drag_stopped() || (!resp.is_pointer_button_down_on() && is_down) {
                            self.note_off(note);
                        }
                    }
                });

                ui.separator();

                // 5. Teclado de Piano Virtual (Sintetizador en Vivo)
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("SINTETIZADOR VIRTUAL (Teclas de Piano)")
                            .size(12.0)
                            .color(Color32::from_rgb(160, 165, 175))
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+").clicked() && self.octave < 7 {
                            self.octave += 1;
                        }
                        ui.label(format!("Octava: C{}", self.octave));
                        if ui.button("-").clicked() && self.octave > 1 {
                            self.octave -= 1;
                        }
                    });
                });

                // Dibujar 14 teclas blancas con sus teclas negras correspondientes
                let base_midi = (self.octave as u8 + 1) * 12; // C_octave
                let white_notes = [0, 2, 4, 5, 7, 9, 11, 12, 14, 16, 17, 19, 21, 23];
                let black_notes = [
                    (0, 1),   // C#
                    (1, 3),   // D#
                    (3, 6),   // F#
                    (4, 8),   // G#
                    (5, 10),  // A#
                    (7, 13),  // C#2
                    (8, 15),  // D#2
                    (10, 18), // F#2
                    (11, 20), // G#2
                    (12, 22), // A#2
                ];

                let (response, painter) = ui.allocate_painter(
                    Vec2::new(ui.available_width(), 130.0),
                    Sense::drag(),
                );

                let rect = response.rect;
                let num_white = white_notes.len() as f32;
                let key_w = rect.width() / num_white;
                let white_h = rect.height();
                let black_h = white_h * 0.62;
                let black_w = key_w * 0.64;

                let mut current_down: HashSet<u8> = HashSet::new();

                if let Some(pos) = response.interact_pointer_pos() {
                    if rect.contains(pos) {
                        // Detectar primero si toca una tecla negra
                        let mut hit_black = false;
                        for (w_idx, semitone) in black_notes {
                            let note = base_midi + semitone;
                            let bx = rect.min.x + (w_idx as f32 + 1.0) * key_w - (black_w / 2.0);
                            let black_rect = Rect::from_min_size(
                                Pos2::new(bx, rect.min.y),
                                Vec2::new(black_w, black_h),
                            );
                            if black_rect.contains(pos) {
                                current_down.insert(note);
                                hit_black = true;
                                break;
                            }
                        }

                        // Si no tocó tecla negra, ver qué tecla blanca tocó
                        if !hit_black {
                            let rel_x = pos.x - rect.min.x;
                            let w_idx = (rel_x / key_w).floor() as usize;
                            if w_idx < white_notes.len() {
                                let note = base_midi + white_notes[w_idx];
                                current_down.insert(note);
                            }
                        }
                    }
                }

                // Sincronizar eventos note_on / note_off
                for &note in &current_down {
                    if !self.active_keys.contains(&note) {
                        self.note_on(note, 100);
                    }
                }
                let released: Vec<u8> = self
                    .active_keys
                    .iter()
                    .copied()
                    .filter(|n| !current_down.contains(n) && *n >= base_midi && *n <= base_midi + 24)
                    .collect();
                for note in released {
                    self.note_off(note);
                }

                // Dibujar Teclas Blancas
                for (i, semitone) in white_notes.iter().enumerate() {
                    let note = base_midi + semitone;
                    let kx = rect.min.x + i as f32 * key_w;
                    let key_rect = Rect::from_min_size(
                        Pos2::new(kx, rect.min.y),
                        Vec2::new(key_w - 1.0, white_h),
                    );

                    let fill = if self.active_keys.contains(&note) {
                        Color32::from_rgb(180, 220, 255)
                    } else {
                        Color32::from_rgb(245, 245, 250)
                    };

                    painter.rect_filled(key_rect, CornerRadius::same(3), fill);
                    painter.rect_stroke(
                        key_rect,
                        CornerRadius::same(3),
                        Stroke::new(1.0, Color32::from_rgb(160, 160, 170)),
                        egui::StrokeKind::Inside,
                    );
                }

                // Dibujar Teclas Negras
                for (w_idx, semitone) in black_notes {
                    let note = base_midi + semitone;
                    let bx = rect.min.x + (w_idx as f32 + 1.0) * key_w - (black_w / 2.0);
                    let black_rect = Rect::from_min_size(
                        Pos2::new(bx, rect.min.y),
                        Vec2::new(black_w, black_h),
                    );

                    let fill = if self.active_keys.contains(&note) {
                        Color32::from_rgb(80, 160, 240)
                    } else {
                        Color32::from_rgb(25, 25, 28)
                    };

                    painter.rect_filled(black_rect, CornerRadius::same(2), fill);
                    painter.rect_stroke(
                        black_rect,
                        CornerRadius::same(2),
                        Stroke::new(1.0, Color32::BLACK),
                        egui::StrokeKind::Inside,
                    );
                }
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
