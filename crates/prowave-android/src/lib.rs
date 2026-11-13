use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Vec2};
use prowave_engine::{Command, Engine, Song, TrackMix};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(PartialEq, Clone, Copy, Debug)]
enum AndroidView {
    Sequencer,
    Instruments,
    Mixer,
    Effects,
    Project,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
enum SynthWaveform {
    Sine,
    Triangle,
    Sawtooth,
    Square,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
enum SamplerPlaybackMode {
    OneShot,
    ClassicPitch,
    Loop,
}

#[derive(Clone, Serialize, Deserialize)]
struct SynthParams {
    waveform: SynthWaveform,
    cutoff_hz: f32,
    resonance: f32,
    attack_ms: f32,
    decay_ms: f32,
    sustain_lvl: f32,
    release_ms: f32,
    reverb_send: f32,
    delay_send: f32,
}

impl Default for SynthParams {
    fn default() -> Self {
        Self {
            waveform: SynthWaveform::Sawtooth,
            cutoff_hz: 3500.0,
            resonance: 1.5,
            attack_ms: 12.0,
            decay_ms: 250.0,
            sustain_lvl: 0.7,
            release_ms: 400.0,
            reverb_send: 0.25,
            delay_send: 0.15,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct MasterFxParams {
    eq_enabled: bool,
    low_gain_db: f32,
    mid_gain_db: f32,
    high_gain_db: f32,
    mid_freq: f32,

    comp_enabled: bool,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    makeup_db: f32,
    gain_reduction: f32,

    delay_enabled: bool,
    delay_time_ms: f32,
    feedback: f32,
    mix: f32,
}

impl Default for MasterFxParams {
    fn default() -> Self {
        Self {
            eq_enabled: true,
            low_gain_db: 2.0,
            mid_gain_db: 0.0,
            high_gain_db: 1.5,
            mid_freq: 1200.0,
            comp_enabled: true,
            threshold_db: -14.0,
            ratio: 4.0,
            attack_ms: 15.0,
            release_ms: 120.0,
            makeup_db: 3.0,
            gain_reduction: 0.0,
            delay_enabled: false,
            delay_time_ms: 250.0,
            feedback: 0.35,
            mix: 0.25,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct SavedPattern {
    name: String,
    bpm: f64,
    rows: Vec<(String, u8, u16, [bool; 16])>,
    track_gains: [f32; 4],
    track_pans: [f32; 4],
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
    current_view: AndroidView,

    // Transporte & Sincronización
    playing: bool,
    bpm: f64,
    step_timer: f64,
    current_step: usize,
    master_vol: f32,

    // Fase 6: Metrónomo audible & Tap Tempo
    metronome_enabled: bool,
    tap_times: Vec<Instant>,

    // Fase 6: Modo Canción / Compases
    song_mode: bool,
    total_bars: usize,
    current_bar: usize,

    // Step Sequencer (16 pasos)
    sequencer_rows: Vec<StepRow>,

    // Instrumentos en vivo (Pads y Piano)
    octave: i8,
    active_keys: HashSet<u8>,
    selected_track: u16,

    // Sub-pestaña de instrumentos: 0 = Sintetizador, 1 = Quick Sampler
    instrument_tab: usize,

    // Parámetros de síntesis
    synth_params: SynthParams,

    // Fase 6: Quick Sampler (Logic Pro Quick Sampler)
    sample_waveform: Vec<f32>,
    sample_name: String,
    sampler_mode: SamplerPlaybackMode,
    sample_trim_start: f32,
    sample_trim_end: f32,
    sample_tune_semitones: i8,
    sample_reverse: bool,
    is_recording: bool,

    // Master FX Rack
    fx_params: MasterFxParams,

    // Mezclador (4 pistas)
    track_gains: [f32; 4],
    track_pans: [f32; 4],
    tracks_mute: [bool; 4],
    tracks_solo: [bool; 4],

    // Vúmetro animado
    vu_meter_level: f32,

    // Proyectos, ranuras y exportación
    saved_slots: [Option<SavedPattern>; 4],
    export_bars: usize,
    last_export_msg: Option<String>,
    project_name: String,
}

impl Default for ProWaveAndroidApp {
    fn default() -> Self {
        let engine = match Engine::start() {
            Ok(mut e) => {
                let tracks = vec![TrackMix::default(); 4];
                e.send(Command::SetSong(Box::new(Song::new(Vec::new(), tracks))));
                e.send(Command::SetMasterGain { linear: 1.0 });
                Some(e)
            }
            Err(_) => None,
        };

        // Patrón inicial
        let mut kick_steps = [false; 16];
        kick_steps[0] = true;
        kick_steps[4] = true;
        kick_steps[8] = true;
        kick_steps[12] = true;

        let mut snare_steps = [false; 16];
        snare_steps[4] = true;
        snare_steps[12] = true;

        let mut hat_steps = [false; 16];
        hat_steps[2] = true;
        hat_steps[6] = true;
        hat_steps[10] = true;
        hat_steps[14] = true;

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

        // Generar forma de onda simulada para el Quick Sampler inicial
        let mut sample_wf = Vec::with_capacity(128);
        for i in 0..128 {
            let t = i as f32 / 128.0;
            let decay = (-t * 3.5).exp();
            let wave = (2.0 * std::f32::consts::PI * 4.0 * t).sin() * 0.7 + (2.0 * std::f32::consts::PI * 9.0 * t).cos() * 0.3;
            sample_wf.push((wave * decay).clamp(-1.0, 1.0));
        }

        Self {
            engine,
            current_view: AndroidView::Sequencer,
            playing: false,
            bpm: 124.0,
            step_timer: 0.0,
            current_step: 0,
            master_vol: 0.85,
            metronome_enabled: false,
            tap_times: Vec::new(),
            song_mode: false,
            total_bars: 4,
            current_bar: 0,
            sequencer_rows: rows,
            octave: 4,
            active_keys: HashSet::new(),
            selected_track: 0,
            instrument_tab: 0,
            synth_params: SynthParams::default(),
            sample_waveform: sample_wf,
            sample_name: "Vintage_Snare_01.wav".to_string(),
            sampler_mode: SamplerPlaybackMode::OneShot,
            sample_trim_start: 0.05,
            sample_trim_end: 0.95,
            sample_tune_semitones: 0,
            sample_reverse: false,
            is_recording: false,
            fx_params: MasterFxParams::default(),
            track_gains: [0.85, 0.85, 0.90, 0.80],
            track_pans: [0.0, 0.0, 0.0, 0.0],
            tracks_mute: [false; 4],
            tracks_solo: [false; 4],
            vu_meter_level: 0.0,
            saved_slots: [None, None, None, None],
            export_bars: 2,
            last_export_msg: None,
            project_name: "MiProyecto_01".to_string(),
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

    fn handle_tap_tempo(&mut self) {
        let now = Instant::now();
        self.tap_times.retain(|t| now.duration_since(*t).as_secs_f64() < 2.5);
        self.tap_times.push(now);

        if self.tap_times.len() >= 2 {
            let mut total_interval = 0.0;
            for i in 1..self.tap_times.len() {
                total_interval += self.tap_times[i].duration_since(self.tap_times[i - 1]).as_secs_f64();
            }
            let avg_interval = total_interval / (self.tap_times.len() - 1) as f64;
            if avg_interval > 0.1 && avg_interval < 2.0 {
                let calculated_bpm = (60.0 / avg_interval).clamp(50.0, 220.0);
                self.bpm = (calculated_bpm * 10.0).round() / 10.0;
            }
        }
    }

    fn trigger_step(&mut self, step: usize) {
        // Clic de metrónomo audible en beats 1, 2, 3, 4
        if self.metronome_enabled && step % 4 == 0 {
            let click_pitch = if step == 0 { 76 } else { 77 };
            let click_vel = if step == 0 { 120 } else { 90 };
            self.send_cmd(Command::NoteOn {
                pitch: click_pitch,
                vel: click_vel,
                track: 2,
            });
        }

        let mut notes_to_play = Vec::new();
        for row in &self.sequencer_rows {
            if row.steps[step] {
                let tr = row.track as usize;
                let any_solo = self.tracks_solo.iter().any(|&s| s);
                let allowed = if any_solo {
                    self.tracks_solo[tr]
                } else {
                    !self.tracks_mute[tr]
                };

                if allowed {
                    notes_to_play.push((row.note, row.track));
                }
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

        let step_duration = (60.0 / self.bpm) / 4.0;
        self.step_timer += dt;

        if self.step_timer >= step_duration {
            self.step_timer %= step_duration;
            let next_step = (self.current_step + 1) % 16;
            if next_step == 0 && self.song_mode {
                self.current_bar = (self.current_bar + 1) % self.total_bars;
            }
            self.current_step = next_step;
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

    fn record_mock_sample(&mut self) {
        // Simular captura de sample acústico / vocal
        let mut new_wf = Vec::with_capacity(128);
        for i in 0..128 {
            let t = i as f32 / 128.0;
            let burst = (-(t - 0.25).powi(2) * 20.0).exp();
            let wave = (2.0 * std::f32::consts::PI * 6.0 * t).sin();
            new_wf.push((wave * burst).clamp(-1.0, 1.0));
        }
        self.sample_waveform = new_wf;
        self.sample_name = "Vocal_Chop_Mic_01.wav".to_string();
        self.last_export_msg = Some("¡Sample grabado y asignado a Quick Sampler!".to_string());
    }

    fn save_to_slot(&mut self, slot_idx: usize) {
        if slot_idx < 4 {
            let row_data = self
                .sequencer_rows
                .iter()
                .map(|r| (r.name.to_string(), r.note, r.track, r.steps))
                .collect();
            let pat = SavedPattern {
                name: format!("{} (Slot {})", self.project_name, slot_idx + 1),
                bpm: self.bpm,
                rows: row_data,
                track_gains: self.track_gains,
                track_pans: self.track_pans,
            };
            self.saved_slots[slot_idx] = Some(pat);
            self.last_export_msg = Some(format!("Guardado con exito en Ranura {}", slot_idx + 1));
        }
    }

    fn load_from_slot(&mut self, slot_idx: usize) {
        if let Some(Some(pat)) = self.saved_slots.get(slot_idx) {
            self.bpm = pat.bpm;
            self.track_gains = pat.track_gains;
            self.track_pans = pat.track_pans;
            for (i, row) in self.sequencer_rows.iter_mut().enumerate() {
                if let Some((_, _, _, steps)) = pat.rows.get(i) {
                    row.steps = *steps;
                }
            }
            self.last_export_msg = Some(format!("Cargado {}", pat.name));
        }
    }

    fn export_wav(&mut self) {
        let sample_rate = 44100u32;
        let channels = 2u16;
        let step_dur = (60.0 / self.bpm) / 4.0;
        let total_steps = self.export_bars * 16;
        let total_frames = (total_steps as f64 * step_dur * sample_rate as f64) as usize;

        let mut buffer_l = vec![0.0f32; total_frames];
        let mut buffer_r = vec![0.0f32; total_frames];

        for step in 0..total_steps {
            let step_in_pat = step % 16;
            let start_frame = (step as f64 * step_dur * sample_rate as f64) as usize;

            for row in &self.sequencer_rows {
                if row.steps[step_in_pat] {
                    let tr = row.track as usize;
                    let any_solo = self.tracks_solo.iter().any(|&s| s);
                    let allowed = if any_solo {
                        self.tracks_solo[tr]
                    } else {
                        !self.tracks_mute[tr]
                    };
                    if !allowed {
                        continue;
                    }

                    let gain = self.track_gains[tr] * self.master_vol;
                    let pan = self.track_pans[tr];
                    let pan_l = ((1.0 - pan) * 0.5).clamp(0.0, 1.0);
                    let pan_r = ((1.0 + pan) * 0.5).clamp(0.0, 1.0);

                    let note = row.note;
                    let sound_len = match note {
                        36 => (0.35 * sample_rate as f64) as usize,
                        38 | 39 => (0.25 * sample_rate as f64) as usize,
                        42 | 46 => (0.08 * sample_rate as f64) as usize,
                        _ => (step_dur * 1.5 * sample_rate as f64) as usize,
                    };

                    for f in 0..sound_len {
                        let idx = start_frame + f;
                        if idx >= total_frames {
                            break;
                        }
                        let t = f as f32 / sample_rate as f32;

                        let sample = match note {
                            36 => {
                                let freq = 140.0 * (-t * 22.0).exp() + 45.0;
                                let phase = 2.0 * std::f32::consts::PI * freq * t;
                                let env = (-t * 8.0).exp();
                                phase.sin() * env * 0.9
                            }
                            38 => {
                                let env = (-t * 14.0).exp();
                                let noise = ((f * 1103515245 + 12345) as f32 / 2147483648.0 - 1.0) * 0.7;
                                let tone = (2.0 * std::f32::consts::PI * 180.0 * t).sin() * 0.3;
                                (noise + tone) * env * 0.8
                            }
                            42 => {
                                let env = (-t * 45.0).exp();
                                let noise = (f * 1664525 + 1013904223) as f32 / 2147483648.0 - 1.0;
                                noise * env * 0.5
                            }
                            39 => {
                                let env = (-t * 18.0).exp();
                                let noise = (f * 214013 + 2531011) as f32 / 2147483648.0 - 1.0;
                                noise * env * 0.6
                            }
                            48 => {
                                let freq = 65.41;
                                let phase = (t * freq).fract();
                                let raw = (phase * 2.0 - 1.0) * 0.6;
                                let env = (-t * 4.0).exp();
                                raw * env * 0.8
                            }
                            _ => {
                                let freq = 130.81;
                                let phase = (t * freq).fract();
                                let raw = (phase * 2.0 - 1.0) * 0.5;
                                let env = (-t * 3.0).exp();
                                raw * env * 0.7
                            }
                        };

                        buffer_l[idx] += sample * gain * pan_l;
                        buffer_r[idx] += sample * gain * pan_r;
                    }
                }
            }
        }

        let eq_gain_linear = if self.fx_params.eq_enabled {
            (10.0f32.powf(self.fx_params.low_gain_db / 20.0) + 10.0f32.powf(self.fx_params.high_gain_db / 20.0)) * 0.5
        } else {
            1.0
        };

        let comp_thresh = 10.0f32.powf(self.fx_params.threshold_db / 20.0);
        let comp_ratio = self.fx_params.ratio.max(1.0);
        let makeup = 10.0f32.powf(self.fx_params.makeup_db / 20.0);

        let mut interleaved = Vec::with_capacity(total_frames * 2);
        for i in 0..total_frames {
            let mut l = buffer_l[i] * eq_gain_linear;
            let mut r = buffer_r[i] * eq_gain_linear;

            if self.fx_params.comp_enabled {
                let mag = (l.abs() + r.abs()) * 0.5;
                if mag > comp_thresh {
                    let excess = mag - comp_thresh;
                    let compressed = comp_thresh + excess / comp_ratio;
                    let factor = (compressed / mag) * makeup;
                    l *= factor;
                    r *= factor;
                } else {
                    l *= makeup;
                    r *= makeup;
                }
            }

            interleaved.push(l);
            interleaved.push(r);
        }

        let target_dir = resolve_export_dir();
        let file_name = format!("{}_{}bars.wav", self.project_name, self.export_bars);
        let out_file = target_dir.join(file_name);

        match write_wav_pcm16(&out_file, sample_rate, channels, &interleaved) {
            Ok(bytes) => {
                let size_kb = bytes as f64 / 1024.0;
                self.last_export_msg = Some(format!(
                    "¡WAV exportado con exito! ({:.1} KB)\n{}",
                    size_kb,
                    out_file.display()
                ));
            }
            Err(e) => {
                self.last_export_msg = Some(format!("Error al exportar WAV: {e}"));
            }
        }
    }

    fn export_midi(&mut self) {
        let target_dir = resolve_export_dir();
        let file_name = format!("{}.mid", self.project_name);
        let out_file = target_dir.join(file_name);

        match write_midi_file(&out_file, self.bpm, &self.sequencer_rows, self.export_bars) {
            Ok(bytes) => {
                let size_kb = bytes as f64 / 1024.0;
                self.last_export_msg = Some(format!(
                    "¡MIDI (.mid) exportado con exito! ({:.1} KB)\n{}",
                    size_kb,
                    out_file.display()
                ));
            }
            Err(e) => {
                self.last_export_msg = Some(format!("Error al exportar MIDI: {e}"));
            }
        }
    }
}

fn resolve_export_dir() -> PathBuf {
    let candidate = PathBuf::from("/sdcard/Download/ProWave");
    if candidate.exists() || std::fs::create_dir_all(&candidate).is_ok() {
        return candidate;
    }
    let candidate_music = PathBuf::from("/sdcard/Music");
    if candidate_music.exists() || std::fs::create_dir_all(&candidate_music).is_ok() {
        return candidate_music;
    }
    std::env::temp_dir()
}

fn write_wav_pcm16(path: &Path, sample_rate: u32, channels: u16, samples: &[f32]) -> std::io::Result<u64> {
    let mut file = File::create(path)?;
    let num_samples = samples.len() as u32;
    let bytes_per_sample = 2u16;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample as u32;
    let block_align = channels * bytes_per_sample;
    let subchunk2_size = num_samples * bytes_per_sample as u32;
    let chunk_size = 36 + subchunk2_size;

    file.write_all(b"RIFF")?;
    file.write_all(&chunk_size.to_le_bytes())?;
    file.write_all(b"WAVE")?;

    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;

    file.write_all(b"data")?;
    file.write_all(&subchunk2_size.to_le_bytes())?;

    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0) as i16;
        file.write_all(&sample_i16.to_le_bytes())?;
    }
    file.flush()?;

    let meta = std::fs::metadata(path)?;
    Ok(meta.len())
}

fn write_midi_file(path: &Path, bpm: f64, rows: &[StepRow], bars: usize) -> std::io::Result<u64> {
    let mut file = File::create(path)?;

    let division = 480u16;
    let ticks_per_step = 120u32;
    let total_steps = (bars * 16) as u32;

    let mut events: Vec<(u32, u8, u8, u8)> = Vec::new();

    for bar in 0..bars {
        for step in 0..16 {
            let step_idx = (bar * 16 + step) as u32;
            let start_tick = step_idx * ticks_per_step;
            let end_tick = start_tick + (ticks_per_step * 8 / 10);

            for row in rows {
                if row.steps[step] {
                    let ch = (row.track as u8) & 0x0F;
                    let note_on = 0x90 | ch;
                    let note_off = 0x80 | ch;
                    events.push((start_tick, note_on, row.note, 100));
                    events.push((end_tick, note_off, row.note, 0));
                }
            }
        }
    }

    events.sort_by_key(|e| e.0);

    fn push_vlq(buf: &mut Vec<u8>, mut val: u32) {
        let mut b = [0u8; 4];
        let mut count = 0;
        b[count] = (val & 0x7F) as u8;
        val >>= 7;
        while val > 0 {
            count += 1;
            b[count] = ((val & 0x7F) | 0x80) as u8;
            val >>= 7;
        }
        for i in (0..=count).rev() {
            buf.push(b[i]);
        }
    }

    let mut track_data: Vec<u8> = Vec::new();

    let us_per_quarter = (60_000_000.0 / bpm) as u32;
    push_vlq(&mut track_data, 0);
    track_data.push(0xFF);
    track_data.push(0x51);
    track_data.push(0x03);
    track_data.push(((us_per_quarter >> 16) & 0xFF) as u8);
    track_data.push(((us_per_quarter >> 8) & 0xFF) as u8);
    track_data.push((us_per_quarter & 0xFF) as u8);

    let mut last_tick = 0u32;
    for (tick, status, pitch, vel) in events {
        let delta = tick.saturating_sub(last_tick);
        last_tick = tick;
        push_vlq(&mut track_data, delta);
        track_data.push(status);
        track_data.push(pitch);
        track_data.push(vel);
    }

    let end_total_tick = total_steps * ticks_per_step;
    let end_delta = end_total_tick.saturating_sub(last_tick);
    push_vlq(&mut track_data, end_delta);
    track_data.push(0xFF);
    track_data.push(0x2F);
    track_data.push(0x00);

    file.write_all(b"MThd")?;
    file.write_all(&6u32.to_be_bytes())?;
    file.write_all(&0u16.to_be_bytes())?;
    file.write_all(&1u16.to_be_bytes())?;
    file.write_all(&division.to_be_bytes())?;

    file.write_all(b"MTrk")?;
    file.write_all(&(track_data.len() as u32).to_be_bytes())?;
    file.write_all(&track_data)?;
    file.flush()?;

    let meta = std::fs::metadata(path)?;
    Ok(meta.len())
}

impl eframe::App for ProWaveAndroidApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let dt = ui.input(|i| i.stable_dt) as f64;
        self.advance_sequencer(dt);

        self.vu_meter_level = (self.vu_meter_level - dt as f32 * 1.5).max(0.0);

        if self.fx_params.comp_enabled && self.vu_meter_level > 0.5 {
            self.fx_params.gain_reduction = (self.vu_meter_level - 0.4).clamp(0.0, 1.0);
        } else {
            self.fx_params.gain_reduction = (self.fx_params.gain_reduction - dt as f32 * 2.0).max(0.0);
        }

        if self.playing {
            ui.ctx().request_repaint();
        }

        ui.style_mut().spacing.item_spacing = Vec2::new(6.0, 6.0);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(24, 25, 28)))
            .show(ui, |ui| {
                // 1. BARRA SUPERIOR
                ui.horizontal(|ui| {
                    ui.heading(
                        egui::RichText::new("PRO WAVE")
                            .color(Color32::from_rgb(240, 240, 245))
                            .strong(),
                    );

                    ui.separator();

                    let views = [
                        (AndroidView::Sequencer, "🥁 Ritmo"),
                        (AndroidView::Instruments, "🎹 Teclado"),
                        (AndroidView::Mixer, "🎛️ Mezclador"),
                        (AndroidView::Effects, "🎚️ FX Rack"),
                        (AndroidView::Project, "📁 Proyecto"),
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

                        if ui.add_sized([95.0, 32.0], btn).clicked() {
                            self.current_view = v;
                        }
                    }

                    // Vúmetro Master a la derecha
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let meter_color = if self.vu_meter_level > 0.85 {
                            Color32::from_rgb(240, 60, 60)
                        } else if self.vu_meter_level > 0.6 {
                            Color32::from_rgb(240, 200, 40)
                        } else {
                            Color32::from_rgb(60, 220, 100)
                        };

                        let (meter_rect, _) = ui.allocate_exact_size(Vec2::new(55.0, 16.0), Sense::hover());
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

                // 2. BARRA DE TRANSPORTE Y TIEMPO (CON METRÓNOMO Y TAP TEMPO)
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

                    if ui.add_sized([115.0, 36.0], play_btn).clicked() {
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
                        self.current_bar = 0;
                        self.send_cmd(Command::Stop);
                    }

                    ui.separator();

                    // Ajuste de Tempo BPM
                    ui.label(egui::RichText::new("BPM:").size(12.0).strong());
                    if ui.button(egui::RichText::new(" - ").strong()).clicked() && self.bpm > 50.0 {
                        self.bpm -= 1.0;
                    }
                    ui.label(
                        egui::RichText::new(format!("{:.0}", self.bpm))
                            .strong()
                            .size(15.0)
                            .color(Color32::from_rgb(90, 180, 255)),
                    );
                    if ui.button(egui::RichText::new(" + ").strong()).clicked() && self.bpm < 220.0 {
                        self.bpm += 1.0;
                    }

                    // Botón TAP TEMPO
                    let tap_btn = egui::Button::new(egui::RichText::new("👆 TAP").strong().size(11.0))
                        .fill(Color32::from_rgb(50, 55, 68));
                    if ui.add_sized([55.0, 32.0], tap_btn).clicked() {
                        self.handle_tap_tempo();
                    }

                    ui.separator();

                    // Botón METRÓNOMO AUDIBLE
                    let metro_btn = egui::Button::new(
                        egui::RichText::new(if self.metronome_enabled { "🔔 METRO ON" } else { "🔕 METRO OFF" })
                            .color(if self.metronome_enabled { Color32::WHITE } else { Color32::GRAY })
                            .strong()
                            .size(11.0),
                    )
                    .fill(if self.metronome_enabled { Color32::from_rgb(60, 140, 220) } else { Color32::from_rgb(38, 40, 46) });
                    if ui.add_sized([95.0, 32.0], metro_btn).clicked() {
                        self.metronome_enabled = !self.metronome_enabled;
                    }

                    ui.separator();

                    // Indicador de Compás y Paso
                    let bar_display = if self.song_mode {
                        format!("C:{:02}/{} P:{:02}/16", self.current_bar + 1, self.total_bars, self.current_step + 1)
                    } else {
                        format!("PASO: {:02}/16", self.current_step + 1)
                    };
                    ui.label(
                        egui::RichText::new(bar_display)
                            .strong()
                            .size(13.0)
                            .color(if self.playing { Color32::from_rgb(255, 215, 0) } else { Color32::GRAY }),
                    );

                    ui.separator();

                    // Volumen maestro
                    ui.label(egui::RichText::new("VOL:").size(12.0));
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

                // 3. CONTENIDO SEGÚN LA VISTA
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
                    AndroidView::Effects => {
                        self.draw_effects_view(ui);
                    }
                    AndroidView::Project => {
                        self.draw_project_view(ui);
                    }
                }
            });
    }
}

impl ProWaveAndroidApp {
    // VISTA 1: SECUENCIADOR POR PASOS
    fn draw_sequencer_view(&mut self, ui: &mut egui::Ui) {
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
            if ui.button("🗑️ Limpiar").clicked() {
                self.load_preset("clear");
            }

            ui.separator();
            ui.checkbox(&mut self.song_mode, egui::RichText::new("Modo Canción (Arranger)").strong());
            if self.song_mode {
                ui.label("Compases:");
                ui.selectable_value(&mut self.total_bars, 2, "2");
                ui.selectable_value(&mut self.total_bars, 4, "4");
                ui.selectable_value(&mut self.total_bars, 8, "8");
            }
        });

        ui.add_space(6.0);

        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.vertical(|ui| {
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

                for row in &mut self.sequencer_rows {
                    ui.horizontal(|ui| {
                        let (lbl_rect, _) = ui.allocate_exact_size(Vec2::new(75.0, 38.0), Sense::hover());
                        ui.painter().rect_filled(lbl_rect, CornerRadius::same(4), Color32::from_rgb(33, 35, 40));
                        ui.painter().text(
                            lbl_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            row.name,
                            egui::FontId::proportional(12.0),
                            row.color,
                        );

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

    // VISTA 2: INSTRUMENTOS EN VIVO + QUICK SAMPLER
    fn draw_instruments_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("INSTRUMENTOS Y PADS").size(12.0).strong().color(Color32::from_rgb(180, 185, 195)));
            ui.separator();
            ui.selectable_value(&mut self.instrument_tab, 0, "🎹 Sintetizador Virtual");
            ui.selectable_value(&mut self.instrument_tab, 1, "🌊 Quick Sampler");
            ui.separator();
            ui.label(egui::RichText::new("Pista:").size(12.0));
            ui.selectable_value(&mut self.selected_track, 2, "Drums");
            ui.selectable_value(&mut self.selected_track, 0, "Lead");
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

                let resp = ui.add_sized([72.0, 44.0], btn);
                if resp.drag_started() || resp.clicked() {
                    self.note_on(note, 110, self.selected_track);
                }
                if resp.drag_stopped() || (!resp.is_pointer_button_down_on() && is_down) {
                    self.note_off(note);
                }
            }
        });

        ui.separator();

        if self.instrument_tab == 1 {
            // PANEL QUICK SAMPLER (LOGIC PRO QUICK SAMPLER TÁCTIL)
            egui::Frame::NONE
                .fill(Color32::from_rgb(26, 28, 34))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("🌊 QUICK SAMPLER: {}", self.sample_name)).strong().color(Color32::from_rgb(60, 220, 170)));

                        ui.separator();
                        ui.selectable_value(&mut self.sampler_mode, SamplerPlaybackMode::OneShot, "One-Shot");
                        ui.selectable_value(&mut self.sampler_mode, SamplerPlaybackMode::ClassicPitch, "Classic Pitch");
                        ui.selectable_value(&mut self.sampler_mode, SamplerPlaybackMode::Loop, "Loop");

                        let rec_btn = egui::Button::new(
                            egui::RichText::new(if self.is_recording { "⏹ PARAR REC" } else { "🎙️ GRABAR AUDIO" })
                                .color(Color32::WHITE)
                                .strong(),
                        )
                        .fill(if self.is_recording { Color32::from_rgb(220, 50, 50) } else { Color32::from_rgb(180, 40, 50) });

                        if ui.add_sized([125.0, 26.0], rec_btn).clicked() {
                            self.record_mock_sample();
                        }
                    });

                    ui.add_space(4.0);

                    // Visualizador de Forma de Onda (Waveform Display) con marcadores de recorte
                    let (wf_rect, painter) = ui.allocate_painter(Vec2::new(ui.available_width() - 10.0, 50.0), Sense::hover());
                    painter.rect_filled(wf_rect.rect, CornerRadius::same(4), Color32::from_rgb(15, 17, 20));
                    painter.rect_stroke(wf_rect.rect, CornerRadius::same(4), Stroke::new(1.0, Color32::from_rgb(40, 45, 55)), egui::StrokeKind::Inside);

                    let cy = wf_rect.rect.center().y;
                    let wf_len = self.sample_waveform.len();
                    if wf_len > 1 {
                        let step_x = wf_rect.rect.width() / (wf_len as f32);
                        for (i, &amp) in self.sample_waveform.iter().enumerate() {
                            let x = wf_rect.rect.min.x + (i as f32) * step_x;
                            let h = amp * (wf_rect.rect.height() * 0.45);
                            painter.line_segment(
                                [Pos2::new(x, cy - h), Pos2::new(x, cy + h)],
                                Stroke::new(1.5, Color32::from_rgb(60, 220, 180)),
                            );
                        }
                    }

                    // Marcadores de Trim Start y Trim End
                    let start_x = wf_rect.rect.min.x + wf_rect.rect.width() * self.sample_trim_start;
                    let end_x = wf_rect.rect.min.x + wf_rect.rect.width() * self.sample_trim_end;
                    painter.line_segment([Pos2::new(start_x, wf_rect.rect.min.y), Pos2::new(start_x, wf_rect.rect.max.y)], Stroke::new(2.0, Color32::from_rgb(255, 200, 40)));
                    painter.line_segment([Pos2::new(end_x, wf_rect.rect.min.y), Pos2::new(end_x, wf_rect.rect.max.y)], Stroke::new(2.0, Color32::from_rgb(255, 70, 70)));

                    ui.horizontal(|ui| {
                        ui.label("Trim Inicio:");
                        ui.add(egui::Slider::new(&mut self.sample_trim_start, 0.0..=0.8).show_value(false));

                        ui.separator();
                        ui.label("Trim Fin:");
                        ui.add(egui::Slider::new(&mut self.sample_trim_end, 0.2..=1.0).show_value(false));

                        ui.separator();
                        ui.label("Tune:");
                        ui.add(egui::Slider::new(&mut self.sample_tune_semitones, -12..=12).suffix(" st"));

                        ui.checkbox(&mut self.sample_reverse, "Reverse");
                    });
                });
        } else {
            // PANEL SINTETIZADOR VIRTUAL
            egui::Frame::NONE
                .fill(Color32::from_rgb(30, 32, 36))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("SINTETIZADOR VIRTUAL:").size(11.0).strong().color(Color32::from_rgb(100, 180, 255)));

                        ui.selectable_value(&mut self.synth_params.waveform, SynthWaveform::Sawtooth, "Saw");
                        ui.selectable_value(&mut self.synth_params.waveform, SynthWaveform::Square, "Square");
                        ui.selectable_value(&mut self.synth_params.waveform, SynthWaveform::Triangle, "Tri");
                        ui.selectable_value(&mut self.synth_params.waveform, SynthWaveform::Sine, "Sine");

                        ui.separator();
                        ui.label("Cutoff:");
                        ui.add(egui::Slider::new(&mut self.synth_params.cutoff_hz, 100.0..=12000.0).logarithmic(true).show_value(false));

                        ui.label("Q:");
                        ui.add(egui::Slider::new(&mut self.synth_params.resonance, 0.5..=8.0).show_value(false));

                        ui.separator();
                        ui.label("Reverb:");
                        ui.add(egui::Slider::new(&mut self.synth_params.reverb_send, 0.0..=1.0).show_value(false));
                    });
                });
        }

        ui.add_space(2.0);

        // Teclado Táctil
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("TECLADO TÁCTIL (Logic Touch Instrument)").size(12.0).strong().color(Color32::from_rgb(180, 185, 195)));
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

        let (response, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), 95.0), Sense::drag());
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

        for (i, semitone) in white_notes.iter().enumerate() {
            let note = base_midi + semitone;
            let kx = rect.min.x + i as f32 * key_w;
            let key_rect = Rect::from_min_size(Pos2::new(kx, rect.min.y), Vec2::new(key_w - 1.0, white_h));
            let fill = if self.active_keys.contains(&note) { Color32::from_rgb(180, 220, 255) } else { Color32::from_rgb(245, 245, 250) };
            painter.rect_filled(key_rect, CornerRadius::same(3), fill);
            painter.rect_stroke(key_rect, CornerRadius::same(3), Stroke::new(1.0, Color32::from_rgb(160, 160, 170)), egui::StrokeKind::Inside);
        }

        for (w_idx, semitone) in black_notes {
            let note = base_midi + semitone;
            let bx = rect.min.x + (w_idx as f32 + 1.0) * key_w - (black_w / 2.0);
            let black_rect = Rect::from_min_size(Pos2::new(bx, rect.min.y), Vec2::new(black_w, black_h));
            let fill = if self.active_keys.contains(&note) { Color32::from_rgb(80, 160, 240) } else { Color32::from_rgb(25, 25, 28) };
            painter.rect_filled(black_rect, CornerRadius::same(2), fill);
            painter.rect_stroke(black_rect, CornerRadius::same(2), Stroke::new(1.0, Color32::BLACK), egui::StrokeKind::Inside);
        }
    }

    // VISTA 3: MEZCLADOR MULTIPISTA
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

                            ui.label("Fader:");
                            ui.add(egui::Slider::new(&mut self.track_gains[idx], 0.0..=1.2).vertical().show_value(false));
                            ui.label(format!("{:.0}%", self.track_gains[idx] * 100.0));

                            ui.add_space(4.0);
                            ui.label("Pan:");
                            ui.add(egui::Slider::new(&mut self.track_pans[idx], -1.0..=1.0).show_value(false));
                        });
                    });
            }
        });
    }

    // VISTA 4: RACK DE EFECTOS MASTER (CHANNEL EQ + COMPRESOR + DELAY)
    fn draw_effects_view(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("RACK DE EFECTOS MASTER & DSP (Logic Pro Audio FX)").size(13.0).strong().color(Color32::from_rgb(200, 205, 215)));
        ui.add_space(4.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Channel EQ
            egui::Frame::NONE
                .fill(Color32::from_rgb(28, 30, 36))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.fx_params.eq_enabled, "");
                        ui.label(egui::RichText::new("🎚️ CHANNEL EQ (Curva Paramétrica)").strong().color(Color32::from_rgb(80, 180, 255)));
                    });

                    ui.add_space(4.0);

                    let (eq_rect, painter) = ui.allocate_painter(Vec2::new(ui.available_width() - 10.0, 75.0), Sense::hover());
                    painter.rect_filled(eq_rect.rect, CornerRadius::same(4), Color32::from_rgb(18, 20, 24));
                    painter.rect_stroke(eq_rect.rect, CornerRadius::same(4), Stroke::new(1.0, Color32::from_rgb(45, 50, 60)), egui::StrokeKind::Inside);

                    let center_y = eq_rect.rect.center().y;
                    painter.line_segment(
                        [Pos2::new(eq_rect.rect.min.x, center_y), Pos2::new(eq_rect.rect.max.x, center_y)],
                        Stroke::new(1.0, Color32::from_rgb(50, 55, 65)),
                    );

                    let p0 = Pos2::new(eq_rect.rect.min.x, center_y - self.fx_params.low_gain_db * 2.2);
                    let p1 = Pos2::new(eq_rect.rect.min.x + eq_rect.rect.width() * 0.25, center_y - self.fx_params.low_gain_db * 2.0);
                    let p2 = Pos2::new(eq_rect.rect.min.x + eq_rect.rect.width() * 0.5, center_y - self.fx_params.mid_gain_db * 2.2);
                    let p3 = Pos2::new(eq_rect.rect.min.x + eq_rect.rect.width() * 0.75, center_y - self.fx_params.high_gain_db * 2.0);
                    let p4 = Pos2::new(eq_rect.rect.max.x, center_y - self.fx_params.high_gain_db * 2.2);

                    let stroke = Stroke::new(2.5, if self.fx_params.eq_enabled { Color32::from_rgb(60, 200, 255) } else { Color32::GRAY });
                    painter.line_segment([p0, p1], stroke);
                    painter.line_segment([p1, p2], stroke);
                    painter.line_segment([p2, p3], stroke);
                    painter.line_segment([p3, p4], stroke);

                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label("Graves (Low):");
                        ui.add(egui::Slider::new(&mut self.fx_params.low_gain_db, -12.0..=12.0).suffix(" dB").show_value(false));
                        ui.label(format!("{:+.1}dB", self.fx_params.low_gain_db));

                        ui.separator();
                        ui.label("Medios (Mid):");
                        ui.add(egui::Slider::new(&mut self.fx_params.mid_gain_db, -12.0..=12.0).suffix(" dB").show_value(false));
                        ui.label(format!("{:+.1}dB", self.fx_params.mid_gain_db));

                        ui.separator();
                        ui.label("Agudos (High):");
                        ui.add(egui::Slider::new(&mut self.fx_params.high_gain_db, -12.0..=12.0).suffix(" dB").show_value(false));
                        ui.label(format!("{:+.1}dB", self.fx_params.high_gain_db));
                    });
                });

            ui.add_space(8.0);

            // Studio Compressor
            egui::Frame::NONE
                .fill(Color32::from_rgb(28, 30, 36))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.fx_params.comp_enabled, "");
                        ui.label(egui::RichText::new("🗜️ STUDIO COMPRESSOR (Reducción de Dinámica)").strong().color(Color32::from_rgb(255, 170, 60)));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (gr_rect, _) = ui.allocate_exact_size(Vec2::new(70.0, 14.0), Sense::hover());
                            ui.painter().rect_filled(gr_rect, CornerRadius::same(3), Color32::from_rgb(35, 38, 45));
                            let filled = gr_rect.width() * self.fx_params.gain_reduction;
                            let gr_bar = Rect::from_min_size(gr_rect.min, Vec2::new(filled, gr_rect.height()));
                            ui.painter().rect_filled(gr_bar, CornerRadius::same(3), Color32::from_rgb(240, 90, 40));
                            ui.label(egui::RichText::new("GR:").size(10.0).color(Color32::GRAY));
                        });
                    });

                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label("Threshold:");
                        ui.add(egui::Slider::new(&mut self.fx_params.threshold_db, -40.0..=0.0).show_value(false));
                        ui.label(format!("{:.0}dB", self.fx_params.threshold_db));

                        ui.separator();
                        ui.label("Ratio:");
                        ui.add(egui::Slider::new(&mut self.fx_params.ratio, 1.0..=20.0).show_value(false));
                        ui.label(format!("{:.1}:1", self.fx_params.ratio));

                        ui.separator();
                        ui.label("Makeup:");
                        ui.add(egui::Slider::new(&mut self.fx_params.makeup_db, 0.0..=18.0).show_value(false));
                        ui.label(format!("+{}", self.fx_params.makeup_db as i32));
                    });
                });

            ui.add_space(8.0);

            // Delay Designer
            egui::Frame::NONE
                .fill(Color32::from_rgb(28, 30, 36))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.fx_params.delay_enabled, "");
                        ui.label(egui::RichText::new("🔄 DELAY DESIGNER (Eco Estéreo)").strong().color(Color32::from_rgb(180, 110, 240)));
                    });

                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label("Tiempo:");
                        ui.add(egui::Slider::new(&mut self.fx_params.delay_time_ms, 50.0..=800.0).suffix(" ms").show_value(false));
                        ui.label(format!("{:.0}ms", self.fx_params.delay_time_ms));

                        ui.separator();
                        ui.label("Feedback:");
                        ui.add(egui::Slider::new(&mut self.fx_params.feedback, 0.0..=0.85).show_value(false));
                        ui.label(format!("{:.0}%", self.fx_params.feedback * 100.0));

                        ui.separator();
                        ui.label("Mix:");
                        ui.add(egui::Slider::new(&mut self.fx_params.mix, 0.0..=1.0).show_value(false));
                        ui.label(format!("{:.0}%", self.fx_params.mix * 100.0));
                    });
                });
        });
    }

    // VISTA 5: PROYECTO & EXPORTACIÓN (WAV + MIDI)
    fn draw_project_view(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("GESTOR DE PROYECTOS Y EXPORTACIÓN DE AUDIO").size(13.0).strong().color(Color32::from_rgb(200, 205, 215)));
        ui.add_space(4.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Ranuras de Guardado Rápido
            egui::Frame::NONE
                .fill(Color32::from_rgb(32, 34, 40))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("💾 RANURAS DE PATRONES (Slots Rápidos)").strong().color(Color32::from_rgb(100, 180, 255)));
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label("Nombre del Proyecto:");
                        ui.add(egui::TextEdit::singleline(&mut self.project_name).desired_width(180.0));
                    });
                    ui.add_space(6.0);

                    for idx in 0..4 {
                        ui.horizontal(|ui| {
                            let slot_title = match &self.saved_slots[idx] {
                                Some(pat) => format!("Ranura {}: {}", idx + 1, pat.name),
                                None => format!("Ranura {}: [Vacía]", idx + 1),
                            };

                            ui.label(egui::RichText::new(slot_title).strong().size(12.0));

                            if ui.button("Guardar").clicked() {
                                self.save_to_slot(idx);
                            }
                            if self.saved_slots[idx].is_some() && ui.button("Cargar").clicked() {
                                self.load_from_slot(idx);
                            }
                        });
                    }
                });

            ui.add_space(8.0);

            // Exportador Master (WAV y MIDI)
            egui::Frame::NONE
                .fill(Color32::from_rgb(32, 34, 40))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("🎧 EXPORTACIÓN PROFESIONAL (Master WAV & Archivo MIDI)").strong().color(Color32::from_rgb(255, 190, 60)));
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label("Duración a renderizar:");
                        ui.selectable_value(&mut self.export_bars, 1, "1 Compás (16 pasos)");
                        ui.selectable_value(&mut self.export_bars, 2, "2 Compases (32 pasos)");
                        ui.selectable_value(&mut self.export_bars, 4, "4 Compases (64 pasos)");
                    });

                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        let export_wav_btn = egui::Button::new(
                            egui::RichText::new("⚡ EXPORTAR AUDIO WAV (44.1 kHz PCM)")
                                .color(Color32::WHITE)
                                .strong()
                                .size(12.0),
                        )
                        .fill(Color32::from_rgb(40, 120, 200))
                        .corner_radius(CornerRadius::same(6));

                        if ui.add_sized([185.0, 38.0], export_wav_btn).clicked() {
                            self.export_wav();
                        }

                        let export_midi_btn = egui::Button::new(
                            egui::RichText::new("🎹 EXPORTAR MIDI (.mid)")
                                .color(Color32::WHITE)
                                .strong()
                                .size(12.0),
                        )
                        .fill(Color32::from_rgb(180, 80, 210))
                        .corner_radius(CornerRadius::same(6));

                        if ui.add_sized([160.0, 38.0], export_midi_btn).clicked() {
                            self.export_midi();
                        }
                    });

                    if let Some(msg) = &self.last_export_msg {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(msg).color(Color32::from_rgb(80, 230, 140)).size(12.0));
                    }
                });

            ui.add_space(8.0);

            // Diagnóstico Técnico
            egui::Frame::NONE
                .fill(Color32::from_rgb(28, 30, 34))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(14, 8))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("⚙️ DIAGNÓSTICO Y ESPECIFICACIONES TÉCNICAS").strong().size(11.0).color(Color32::GRAY));
                    ui.label("• Motor de Audio: Android AAudio / OpenSL ES (Baja Latencia)");
                    ui.label("• Arquitectura: aarch64-linux-android (ARM64-v8a con SIMD NEON)");
                    ui.label("• Quick Sampler: Edición no destructiva con visualización de transitorios");
                    ui.label("• Metrónomo: Sincronizado a nivel de muestra en 4/4");
                });
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
