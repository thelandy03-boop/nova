//! Import audio files as new audio tracks (Logic Pro: File > Import > Audio File, or drag-and-drop).
use crate::model::{AudioRegion, Project, Region, RegionParams, Track, TrackKind, TrackParams};
use prowave_engine::{AudioError, AudioFile};
use std::path::Path;

const PEAK_BINS: usize = 4096;

pub fn is_audio_path(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "wav"
                | "wave"
                | "aif"
                | "aiff"
                | "aifc"
                | "flac"
                | "mp3"
                | "caf"
                | "ogg"
                | "oga"
                | "m4a"
                | "mp4"
        )
    })
}

fn peak_cache_path(file: &AudioFile, bins: usize) -> Option<std::path::PathBuf> {
    let meta = std::fs::metadata(&file.path).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |w: u64| h = (h ^ w).wrapping_mul(0x0000_0100_0000_01b3);
    for b in file.path.to_string_lossy().bytes() {
        mix(u64::from(b));
    }
    mix(meta.len());
    mix(modified);
    mix(file.frames);
    mix(file.sample_rate as u64);
    mix(bins as u64);

    let dir = crate::recent_files::config_dir()
        .map(|d| d.join("peak-cache"))
        .unwrap_or_else(|| std::env::temp_dir().join("prowave-peak-cache"));
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join(format!("{h:016x}.peaks")))
}

fn load_peak_cache(file: &AudioFile, bins: usize) -> Option<Vec<prowave_engine::WaveformPeak>> {
    let path = peak_cache_path(file, bins)?;
    let bytes = std::fs::read(path).ok()?;
    let expected = bins * std::mem::size_of::<prowave_engine::WaveformPeak>();
    if bytes.len() != expected {
        return None;
    }
    let mut peaks = Vec::with_capacity(bins);
    unsafe {
        let ptr = bytes.as_ptr() as *const prowave_engine::WaveformPeak;
        peaks.extend_from_slice(std::slice::from_raw_parts(ptr, bins));
    }
    Some(peaks)
}

fn save_peak_cache(file: &AudioFile, bins: usize, peaks: &[prowave_engine::WaveformPeak]) {
    if let Some(path) = peak_cache_path(file, bins) {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                peaks.as_ptr() as *const u8,
                peaks.len() * std::mem::size_of::<prowave_engine::WaveformPeak>(),
            )
        };
        let _ = std::fs::write(path, bytes);
    }
}

/// Wraps an audio file as a region source and computes its waveform overview on a
/// background thread with instant disk-cache lookup and progressive streaming.
pub fn audio_region(file: AudioFile, ctx: &egui::Context) -> AudioRegion {
    let peaks = crate::model::WaveformOverview::new();
    {
        let (file, peaks, ctx) = (file.clone(), peaks.clone(), ctx.clone());
        std::thread::spawn(move || {
            // 1. Instant disk cache lookup (< 1 ms load time)
            if let Some(cached) = load_peak_cache(&file, PEAK_BINS) {
                peaks.update(cached);
                ctx.request_repaint();
                return;
            }

            // 2. Progressive background computation with real-time UI streaming
            let res = file.peaks_progressive(PEAK_BINS, |partial| {
                peaks.update(partial.to_vec());
                ctx.request_repaint();
            });

            match res {
                Ok(v) => {
                    save_peak_cache(&file, PEAK_BINS, &v);
                    peaks.update(v);
                    ctx.request_repaint();
                }
                Err(e) => tracing::warn!("waveform overview failed for {}: {e}", file.path.display()),
            }
        });
    }
    AudioRegion {
        file,
        peaks,
        flex: None,
    }
}

/// Adds a new audio track with one region starting at the bar under the playhead.
pub fn import_audio(p: &mut Project, path: &Path, ctx: &egui::Context) -> Result<(), AudioError> {
    let file = AudioFile::open(path)?;
    let start_bar = p.beats_to_bar(p.position_beats).floor() + 1.0;
    // The region's length follows the Tempo track from where it starts.
    let map = p.tempo_map();
    let start = p.bar_to_beats(start_bar - 1.0);
    let end = map.advance(start, file.duration_secs());
    let len_bars = p.beats_to_bar(end) - (start_bar - 1.0);
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Audio")
        .to_string();

    p.tracks.push(Track {
        name: name.clone(),
        kind: TrackKind::Audio,
        color: egui::Color32::from_rgb(0x3f, 0x5f, 0xc0),
        mute: false,
        solo: false,
        rec: false,
        monitor: false,
        input: prowave_engine::InputSource::Mono(0),
        output: 0,
        volume_db: 0.0,
        pan: 0.0,
        inserts: vec![],
        instrument: None,
        regions: vec![Region {
            name,
            start_bar: start_bar as f32,
            len_bars: len_bars as f32,
            notes: vec![],
            controls: Vec::new(),
            folder: None,
            generated: None,
            audio: Some(audio_region(file, ctx)),
            params: RegionParams::default(),
        }],
        params: TrackParams::default(),
        category: None,
        sends: Vec::new(),
        auto: crate::model::Automation::default(),
        group: None,
        height: 1.0,
        indent: 0,
        folded: false,
        vca: None,
        midi_port: None,
        soundfont: None,
        icon: None,
        midi_channel: 1,
        midi_program: None,
        midi_bank: None,
        midi_fx: Vec::new(),
    });
    p.selected_track = p.tracks.len() - 1;
    p.song_rev += 1;
    Ok(())
}
