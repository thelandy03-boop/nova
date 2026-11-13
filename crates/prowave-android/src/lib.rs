use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Vec2};
use prowave_engine::{Command, Engine, Song, TrackMix};
use std::collections::HashSet;

#[derive(PartialEq, Clone, Copy, Debug)]
enum AndroidView {
    Sequencer,
    Instruments,
    Mixer,
}

struct StepRow {
    name: &'static str,
    note: u8,
    track: u16,
    color: Color32,
    steps: [bool; 16],
}

struct ProWaveAndroidApp {
    engine: Option<Engine>,
    error_msg: Option<String>,
    current_view: AndroidView,

    // Transporte
    playing: bool,
    bpm: f64,
    step_timer: f64,
    current_step: usize,
    metronome: bool,
    test_tone: bool,
    master_vol: f32,

    // Step Sequencer (16 pasos)
    sequencer_rows: Vec<StepRow>,

    // Instrumentos en vivo (Pads y Piano)
    octave: i8,
    active_keys: HashSet<u8>,
    selected_track: u16,

    // Mezclador (4 pistas)
    track_gains: [f32; 4],
    track_pans: [f32; 4],
    tracks_mute: [bool; 4],
    tracks_solo: [bool; 4],

    // Vúmetro animado
    vu_meter_level: f32,
}

impl Default for ProWaveAndroidApp {
    fn default() -> Self {
        let (engine, error_msg) = match Engine::start() {
            Ok(mut e) => {
                let tracks = vec![TrackMix::default(); 4];
                e.send(Command::SetSong(Box::new(Song::new(Vec::new(), tracks))));
                e.send(Command::SetMasterGain { linear: 1.0 });
                (Some(e), None)
            }
            Err(err) => (None, Some(format!("Error de audio: {err}"))),
        };

        // Patrón rítmico inicial clásico (House/Electronic)
        let mut kick_steps = [false; 16];
        kick_steps[0] = true;
        kick_steps[4] = true;
        kick_steps[8] = true;
        kick_steps[12] = true; // Four on the floor

        let mut snare_steps = [false; 16];
        snare_steps[4] = true;
        snare_steps[12] = true; // Beat 2 y 4

        let mut hat_steps = [false; 16];
        hat_steps[2] = true;
        hat_steps[6] = true;
        hat_steps[10] = true;
        hat_steps[14] = true; // Offbeat hats

        let mut bass_steps = [false; 16];
        bass_steps[0] = true;
        bass_steps[3] = true;
        bass_steps[8] = true;
        bass_steps[11] = true;

        let rows = vec![
            StepRow {
                name: "Kick",
                note: 36,
                track: 2,
                color: Color32::from_rgb(230, 60, 60),
                steps: kick_steps,
            },
            StepRow {
                name: "Snare",
                note: 38,
                track: 2,
                color: Color32::from_rgb(240, 140, 40),
                steps: snare_steps,
            },
            StepRow {
                name: "Hi-Hat",
                note: 42,
                track: 2,
                color: Color32::from_rgb(50, 170, 240),
                steps: hat_steps,
            },
            StepRow {
                name: "Clap",
                note: 39,
                track: 2,
                color: Color32::from_rgb(240, 210, 50),
                steps: [false; 16],
            },
            StepRow {
                name: "Bass",
                note: 48,
                track: 1,
                color: Color32::from_rgb(180, 80, 220),
                steps: bass_steps,
            },
            StepRow {
                name: "Synth",
                note: 60,
                track: 0,
                color: Color32::from_rgb(60, 210, 150),
                steps: [false; 16],
            },
        ];

        Self {
            engine,
            error_msg,
            current_view: AndroidView::Sequencer,
            playing: false,
            bpm: 124.0,
            step_timer: 0.0,
            current_step: 0,
            metronome: false,
            test_tone: false,
            master_vol: 0.85,
            sequencer_rows: rows,
            octave: 4,
            active_keys: HashSet::new(),
            selected_track: 0,
            track_gains: [0.85, 0.85, 0.90, 0.80],
            track_pans: [0.0, 0.0, 0.0, 0.0],
            tracks_mute: [false; 4],
            tracks_solo: [false; 4],
            vu_meter_level: 0.0,
        }
    }
}

impl ProWaveAndroidApp {
    fn send_cmd(&mut self, cmd: Command) {
        if let Some(engine) = self.engine.as_mut() {
            engine.send(cmd);
        }
    }

    fn note_on(&mut self, pitch: u8, vel: u8, track: u16) {
        if self.active_keys.insert(pitch) {
            self.send_cmd(Command::NoteOn { pitch, vel, track });
            self.vu_meter_level = (self.vu_meter_level + 0.35).min(1.0);
        }
    }

    fn note_off(&mut self, pitch: u8) {
        if self.active_keys.remove(&pitch) {
            self.send_cmd(Command::NoteOff { pitch });
        }
    }

    fn trigger_step(&mut self, step: usize) {
        let mut notes_to_play = Vec::new();
        for row in &self.sequencer_rows {
            if row.steps[step] {
                notes_to_play.push((row.note, row.track));
            }
        }
        for (note, track) in notes_to_play {
            self.send_cmd(Command::NoteOn {
                pitch: note,
                vel: 115,
                track,
            });
            self.vu_meter_level = (self.vu_meter_level + 0.25).min(1.0);
        }
    }

    fn advance_sequencer(&mut self, dt: f64) {
        if !self.playing {
            return;
        }

        // 16 pasos por compás: cada paso = (60.0 / bpm) / 4 segundos
        let step_duration = (60.0 / self.bpm) / 4.0;
        self.step_timer += dt;

        if self.step_timer >= step_duration {
            self.step_timer %= step_duration;
            self.current_step = (self.current_step + 1) % 16;
            self.trigger_step(self.current_step);
        }
    }

    fn load_preset(&mut self, preset: &str) {
        for row in &mut self.sequencer_rows {
            row.steps = [false; 16];
        }
        match preset {
            "house" => {
                self.bpm = 124.0;
                self.sequencer_rows[0].steps = [true, false, false, false, true, false, false, false, true, false, false, false, true, false, false, false];
                self.sequencer_rows[1].steps = [false, false, false, false, true, false, false, false, false, false, false, false, true, false, false, false];
                self.sequencer_rows[2].steps = [false, false, true, false, false, false, true, false, false, false, true, false, false, false, true, false];
                self.sequencer_rows[4].steps = [true, false, false, true, false, false, true, false, true, false, false, true, false, false, true, false];
            }
            "trap" => {
                self.bpm = 140.0;
                self.sequencer_rows[0].steps = [true, false, false, false, false, false, false, false, true, false, true, false, false, false, false, false];
                self.sequencer_rows[1].steps = [false, false, false, false, false, false, false, false, true, false, false, false, false, false, false, false];
                self.sequencer_rows[2].steps = [true, true, true, true, true, true, true, true, true, true, true, true, true, true, true, true];
                self.sequencer_rows[3].steps = [false, false, false, false, false, false, false, false, true, false, false, false, false, false, false, false];
                self.sequencer_rows[4].steps = [true, false, false, false, false, false, false, false, true, false, false, false, false, false, false, false];
            }
            "hiphop" => {
                self.bpm = 90.0;
                self.sequencer_rows[0].steps = [true, false, false, false, false, false, false, false, false, false, true, false, false, false, false, false];
                self.sequencer_rows[1].steps = [false, false, false, false, true, false, false, false, false, false, false, false, true, false, false, false];
                self.sequencer_rows[2].steps = [true, false, true, false, true, false, true, false, true, false, true, false, true, false, true, false];
                self.sequencer_rows[4].steps = [true, false, false, false, true, false, false, false, true, false, false, false, false, false, false, false];
            }
            _ => {}
        }
    }
}

impl eframe::App for ProWaveAndroidApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let dt = ui.input(|i| i.stable_dt) as f64;
        self.advance_sequencer(dt);

        // Desvanecimiento suave del vúmetro
        self.vu_meter_level = (self.vu_meter_level - dt as f32 * 1.5).max(0.0);

        // Siempre solicitar redibujado continuo cuando está en reproducción para fluidez
        if self.playing {
            ui.ctx().request_repaint();
        }

        ui.style_mut().spacing.item_spacing = Vec2::new(6.0, 6.0);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(24, 25, 28)))
            .show(ui, |ui| {
                // 1. BARRA SUPERIOR: Header + Estado + Pestañas de Vista
                ui.horizontal(|ui| {
                    ui.heading(
                        egui::RichText::new("PRO WAVE")
                            .color(Color32::from_rgb(240, 240, 245))
                            .strong(),
                    );

                    ui.separator();

                    // Pestañas principales
                    let views = [
                        (AndroidView::Sequencer, "🥁 Secuenciador"),
                        (AndroidView::Instruments, "🎹 Instrumentos"),
                        (AndroidView::Mixer, "🎛️ Mezclador"),
                    ];
                    for (v, title) in views {
                        let is_active = self.current_view == v;
                        let btn = egui::Button::new(
                            egui::RichText::new(title)
                                .color(if is_active { Color32::WHITE } else { Color32::from_rgb(160, 165, 175) })
                                .strong(),
                        )
                        .fill(if is_active { Color32::from_rgb(45, 90, 160) } else { Color32::from_rgb(34, 36, 42) })
                        .corner_radius(CornerRadius::same(6));

                        if ui.add_sized([120.0, 32.0], btn).clicked() {
                            self.current_view = v;
                        }
                    }

                    // Estado y Vúmetro Master a la derecha
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Vúmetro visual
                        let meter_color = if self.vu_meter_level > 0.85 {
                            Color32::from_rgb(240, 60, 60)
                        } else if self.vu_meter_level > 0.6 {
                            Color32::from_rgb(240, 200, 40)
                        } else {
                            Color32::from_rgb(60, 220, 100)
                        };

                        let (meter_rect, _) = ui.allocate_exact_size(Vec2::new(70.0, 16.0), Sense::hover());
                        ui.painter().rect_filled(meter_rect, CornerRadius::same(3), Color32::from_rgb(35, 38, 45));
                        let filled_w = meter_rect.width() * self.vu_meter_level;
                        let active_rect = Rect::from_min_size(meter_rect.min, Vec2::new(filled_w, meter_rect.height()));
                        ui.painter().rect_filled(active_rect, CornerRadius::same(3), meter_color);

                        ui.label(egui::RichText::new("NIVEL:").size(10.0).color(Color32::GRAY));

                        if let Some(engine) = &self.engine {
                            let status = format!("AAudio {}Hz", engine.sample_rate());
                            ui.label(egui::RichText::new(status).color(Color32::from_rgb(80, 200, 120)).size(11.0));
                        }
                    });
                });

                ui.separator();

                // 2. BARRA DE TRANSPORTE Y TIEMPO (Siempre visible)
                ui.horizontal(|ui| {
                    let play_color = if self.playing {
                        Color32::from_rgb(45, 180, 80)
                    } else {
                        Color32::from_rgb(38, 40, 46)
                    };

                    let play_btn = egui::Button::new(
                        egui::RichText::new(if self.playing { "⏸ PAUSAR" } else { "▶ REPRODUCIR" })
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(play_color)
                    .corner_radius(CornerRadius::same(6));

                    if ui.add_sized([120.0, 36.0], play_btn).clicked() {
                        self.playing = !self.playing;
                        if self.playing {
                            self.send_cmd(Command::Play);
                        } else {
                            self.send_cmd(Command::Pause);
                        }
                    }

                    let stop_btn = egui::Button::new(
                        egui::RichText::new("⏹ DETENER")
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(Color32::from_rgb(38, 40, 46))
                    .corner_radius(CornerRadius::same(6));

                    if ui.add_sized([90.0, 36.0], stop_btn).clicked() {
                        self.playing = false;
                        self.current_step = 0;
                        self.send_cmd(Command::Stop);
                    }

                    ui.separator();

                    // Ajuste de Tempo BPM
                    ui.label(egui::RichText::new("TEMPO:").size(12.0).strong());
                    if ui.button(egui::RichText::new(" - ").strong()).clicked() && self.bpm > 50.0 {
                        self.bpm -= 1.0;
                    }
                    ui.label(
                        egui::RichText::new(format!("{:.0} BPM", self.bpm))
                            .strong()
                            .size(15.0)
                            .color(Color32::from_rgb(90, 180, 255)),
                    );
                    if ui.button(egui::RichText::new(" + ").strong()).clicked() && self.bpm < 220.0 {
                        self.bpm += 1.0;
                    }

                    ui.separator();

                    // Selector de Paso actual (Display visual)
                    ui.label(egui::RichText::new("PASO:").size(12.0));
                    ui.label(
                        egui::RichText::new(format!("{:02}/16", self.current_step + 1))
                            .strong()
                            .size(15.0)
                            .color(if self.playing { Color32::from_rgb(255, 215, 0) } else { Color32::GRAY }),
                    );

                    ui.separator();

                    // Volumen maestro
                    ui.label(egui::RichText::new("MASTER:").size(12.0));
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

                // 3. CONTENIDO SEGÚN LA PESTAÑA SELECCIONADA
                match self.current_view {
                    AndroidView::Sequencer => {
                        self.draw_sequencer_view(ui);
                    }
                    AndroidView::Instruments => {
                        self.draw_instruments_view(ui);
                    }
                    AndroidView::Mixer => {
                        self.draw_mixer_view(ui);
                    }
                }
            });
    }
}

impl ProWaveAndroidApp {
    // VISTA 1: SECUENCIADOR POR PASOS (16 PASOS)
    fn draw_sequencer_view(&mut self, ui: &mut egui::Ui) {
        // Barra de presets y utilidades
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("RITMOS PREDEFINIDOS:").size(12.0).color(Color32::from_rgb(170, 175, 185)).strong());

            if ui.button("⚡ House 4/4").clicked() {
                self.load_preset("house");
            }
            if ui.button("🔥 Trap Beat").clicked() {
                self.load_preset("trap");
            }
            if ui.button("🎧 Hip-Hop").clicked() {
                self.load_preset("hiphop");
            }
            if ui.button("🗑️ Limpiar Todo").clicked() {
                self.load_preset("clear");
            }
        });

        ui.add_space(6.0);

        // Cuadrícula de 16 pasos con scroll horizontal táctil
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.vertical(|ui| {
                // Cabecera con números de pasos 1..16
                ui.horizontal(|ui| {
                    ui.allocate_exact_size(Vec2::new(75.0, 20.0), Sense::hover());
                    for step in 0..16 {
                        let is_current = self.playing && self.current_step == step;
                        let text_color = if is_current {
                            Color32::from_rgb(255, 220, 40)
                        } else if step % 4 == 0 {
                            Color32::WHITE
                        } else {
                            Color32::GRAY
                        };

                        let (rect, _) = ui.allocate_exact_size(Vec2::new(38.0, 20.0), Sense::hover());
                        if is_current {
                            ui.painter().rect_filled(rect, CornerRadius::same(3), Color32::from_rgb(60, 65, 80));
                        }
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}", step + 1),
                            egui::FontId::proportional(11.0),
                            text_color,
                        );
                    }
                });

                // Filas de instrumentos del secuenciador
                for row in &mut self.sequencer_rows {
                    ui.horizontal(|ui| {
                        // Etiqueta del instrumento
                        let (lbl_rect, _) = ui.allocate_exact_size(Vec2::new(75.0, 38.0), Sense::hover());
                        ui.painter().rect_filled(lbl_rect, CornerRadius::same(4), Color32::from_rgb(33, 35, 40));
                        ui.painter().text(
                            lbl_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            row.name,
                            egui::FontId::proportional(12.0),
                            row.color,
                        );

                        // 16 Botones de pasos
                        for step in 0..16 {
                            let is_on = row.steps[step];
                            let is_current = self.playing && self.current_step == step;

                            let bg_color = if is_on {
                                if is_current {
                                    Color32::WHITE
                                } else {
                                    row.color
                                }
                            } else {
                                if is_current {
                                    Color32::from_rgb(55, 60, 72)
                                } else if step % 4 == 0 {
                                    Color32::from_rgb(38, 40, 46)
                                } else {
                                    Color32::from_rgb(28, 30, 35)
                                }
                            };

                            let (btn_rect, response) = ui.allocate_exact_size(Vec2::new(38.0, 38.0), Sense::click());
                            ui.painter().rect_filled(btn_rect, CornerRadius::same(5), bg_color);

                            // Borde
                            let stroke_color = if is_current {
                                Color32::from_rgb(255, 215, 0)
                            } else {
                                Color32::from_rgb(45, 48, 56)
                            };
                            ui.painter().rect_stroke(btn_rect, CornerRadius::same(5), Stroke::new(1.0, stroke_color), egui::StrokeKind::Inside);

                            if response.clicked() {
                                row.steps[step] = !row.steps[step];
                            }
                        }
                    });
                }
            });
        });
    }

    // VISTA 2: INSTRUMENTOS EN VIVO (DRUM PADS + PIANO)
    fn draw_instruments_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("DRUM MACHINE PADS").size(12.0).strong().color(Color32::from_rgb(180, 185, 195)));
            ui.separator();
            ui.label(egui::RichText::new("Pista destino:").size(12.0));
            ui.selectable_value(&mut self.selected_track, 2, "Drums (Pista 3)");
            ui.selectable_value(&mut self.selected_track, 0, "Synth (Pista 1)");
        });

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
                let bg = if is_down { Color32::WHITE } else { color };

                let btn = egui::Button::new(
                    egui::RichText::new(label)
                        .color(if is_down { Color32::BLACK } else { Color32::WHITE })
                        .size(11.0)
                        .strong(),
                )
                .fill(bg)
                .corner_radius(CornerRadius::same(8));

                let resp = ui.add_sized([72.0, 52.0], btn);
                if resp.drag_started() || resp.clicked() {
                    self.note_on(note, 110, self.selected_track);
                }
                if resp.drag_stopped() || (!resp.is_pointer_button_down_on() && is_down) {
                    self.note_off(note);
                }
            }
        });

        ui.separator();

        // Piano Virtual
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("TECLADO TÁCTIL (Sintetizador)").size(12.0).strong().color(Color32::from_rgb(180, 185, 195)));
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

        let base_midi = (self.octave as u8 + 1) * 12;
        let white_notes = [0, 2, 4, 5, 7, 9, 11, 12, 14, 16, 17, 19, 21, 23];
        let black_notes = [
            (0, 1), (1, 3), (3, 6), (4, 8), (5, 10),
            (7, 13), (8, 15), (10, 18), (11, 20), (12, 22),
        ];

        let (response, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), 120.0), Sense::drag());
        let rect = response.rect;
        let num_white = white_notes.len() as f32;
        let key_w = rect.width() / num_white;
        let white_h = rect.height();
        let black_h = white_h * 0.62;
        let black_w = key_w * 0.64;

        let mut current_down: HashSet<u8> = HashSet::new();

        if let Some(pos) = response.interact_pointer_pos() {
            if rect.contains(pos) {
                let mut hit_black = false;
                for (w_idx, semitone) in black_notes {
                    let note = base_midi + semitone;
                    let bx = rect.min.x + (w_idx as f32 + 1.0) * key_w - (black_w / 2.0);
                    let black_rect = Rect::from_min_size(Pos2::new(bx, rect.min.y), Vec2::new(black_w, black_h));
                    if black_rect.contains(pos) {
                        current_down.insert(note);
                        hit_black = true;
                        break;
                    }
                }
                if !hit_black {
                    let rel_x = pos.x - rect.min.x;
                    let w_idx = (rel_x / key_w).floor() as usize;
                    if w_idx < white_notes.len() {
                        current_down.insert(base_midi + white_notes[w_idx]);
                    }
                }
            }
        }

        for &note in &current_down {
            if !self.active_keys.contains(&note) {
                self.note_on(note, 100, 0);
            }
        }
        let released: Vec<u8> = self.active_keys.iter().copied().filter(|n| !current_down.contains(n) && *n >= base_midi && *n <= base_midi + 24).collect();
        for note in released {
            self.note_off(note);
        }

        // Dibujar Teclas Blancas
        for (i, semitone) in white_notes.iter().enumerate() {
            let note = base_midi + semitone;
            let kx = rect.min.x + i as f32 * key_w;
            let key_rect = Rect::from_min_size(Pos2::new(kx, rect.min.y), Vec2::new(key_w - 1.0, white_h));
            let fill = if self.active_keys.contains(&note) { Color32::from_rgb(180, 220, 255) } else { Color32::from_rgb(245, 245, 250) };
            painter.rect_filled(key_rect, CornerRadius::same(3), fill);
            painter.rect_stroke(key_rect, CornerRadius::same(3), Stroke::new(1.0, Color32::from_rgb(160, 160, 170)), egui::StrokeKind::Inside);
        }

        // Dibujar Teclas Negras
        for (w_idx, semitone) in black_notes {
            let note = base_midi + semitone;
            let bx = rect.min.x + (w_idx as f32 + 1.0) * key_w - (black_w / 2.0);
            let black_rect = Rect::from_min_size(Pos2::new(bx, rect.min.y), Vec2::new(black_w, black_h));
            let fill = if self.active_keys.contains(&note) { Color32::from_rgb(80, 160, 240) } else { Color32::from_rgb(25, 25, 28) };
            painter.rect_filled(black_rect, CornerRadius::same(2), fill);
            painter.rect_stroke(black_rect, CornerRadius::same(2), Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Inside);
        }
    }

    // VISTA 3: MEZCLADOR MULTIPISTA (CANALES + VÚMETROS)
    fn draw_mixer_view(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("MEZCLADOR DE PISTAS (Logic Channel Strips)").size(12.0).strong().color(Color32::from_rgb(180, 185, 195)));
        ui.add_space(4.0);

        let track_names = ["1 · Synth Lead", "2 · Bass Synth", "3 · Drum Kit", "4 · Pad / Strings"];

        ui.horizontal(|ui| {
            for idx in 0..4 {
                let name = track_names[idx];
                let is_sel = self.selected_track == idx as u16;

                egui::Frame::NONE
                    .fill(Color32::from_rgb(30, 32, 37))
                    .stroke(Stroke::new(1.0, if is_sel { Color32::from_rgb(70, 130, 220) } else { Color32::from_rgb(45, 48, 55) }))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_width(120.0);
                        ui.vertical_centered(|ui| {
                            if ui.selectable_label(is_sel, egui::RichText::new(name).strong().size(12.0)).clicked() {
                                self.selected_track = idx as u16;
                            }
                            ui.separator();

                            // Controles Mute / Solo
                            ui.horizontal(|ui| {
                                let m = &mut self.tracks_mute[idx];
                                let mute_btn = egui::Button::new("M")
                                    .fill(if *m { Color32::from_rgb(220, 60, 60) } else { Color32::from_rgb(40, 42, 48) });
                                if ui.add_sized([30.0, 24.0], mute_btn).clicked() {
                                    *m = !*m;
                                }

                                let s = &mut self.tracks_solo[idx];
                                let solo_btn = egui::Button::new("S")
                                    .fill(if *s { Color32::from_rgb(230, 190, 40) } else { Color32::from_rgb(40, 42, 48) });
                                if ui.add_sized([30.0, 24.0], solo_btn).clicked() {
                                    *s = !*s;
                                }
                            });

                            ui.add_space(6.0);

                            // Fader Vertical de Volumen
                            ui.label("Fader:");
                            ui.add(egui::Slider::new(&mut self.track_gains[idx], 0.0..=1.2).vertical().show_value(false));
                            ui.label(format!("{:.0}%", self.track_gains[idx] * 100.0));

                            ui.add_space(4.0);
                            // Pan
                            ui.label("Pan:");
                            ui.add(egui::Slider::new(&mut self.track_pans[idx], -1.0..=1.0).show_value(false));
                        });
                    });
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
