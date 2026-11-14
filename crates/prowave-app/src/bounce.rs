//! File > Bounce > Project or Section…, Export > Track as Audio File… and Bounce In Place: plays
//! the song through the engine and writes what is heard into a WAV file.
//!
//! The engine hands over its finished master frames (after every effect, send, automation lane
//! and the master fader, see `Command::SetCapture`), so the file is exactly the mix. A bounce
//! runs at playing speed: a four-minute song takes four minutes.
//!
//! A bounce of a track or a region is only worth placing if that track or region is still the
//! one that was bounced when it ends. The bounce keeps a copy of what it read, and checks it
//! against the project at the end: a region or track that was edited, moved, deleted or
//! reordered meanwhile is not replaced (or exported) with audio it does not match.

use prowave_engine::{write_wav, CaptureSink, Command, Engine};
use rtrb::{Consumer, RingBuffer};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::model::{Control, Note, Project, Region, RegionParams, Track};

/// The engine's ring holds about 1.4 s at 48 kHz. A collector thread empties it every couple
/// of milliseconds, so the ring only fills if that thread is starved for that long; if it
/// does, the engine counts the frames it lost and the bounce is refused.
const RING_FRAMES: usize = 65_536;

/// How long the collector sleeps when the ring is empty.
const COLLECT_EVERY: Duration = Duration::from_millis(2);

/// The parts of a region that a bounce of it depends on.
#[derive(Clone)]
pub struct RegionContent {
    name: String,
    start_bar: f32,
    len_bars: f32,
    notes: Vec<Note>,
    controls: Vec<Control>,
    params: RegionParams,
    audio: bool,
}

impl RegionContent {
    pub fn of(r: &Region) -> Self {
        Self {
            name: r.name.clone(),
            start_bar: r.start_bar,
            len_bars: r.len_bars,
            notes: r.notes.clone(),
            controls: r.controls.clone(),
            params: r.params,
            audio: r.audio.is_some(),
        }
    }

    /// Whether `r` still has this content.
    pub fn matches(&self, r: &Region) -> bool {
        self.name == r.name
            && self.start_bar == r.start_bar
            && self.len_bars == r.len_bars
            && self.notes == r.notes
            && self.controls == r.controls
            && self.params == r.params
            && self.audio == r.audio.is_some()
    }
}

/// What a bounce reads from the project, to check at its end.
#[derive(Clone)]
pub enum Target {
    /// The whole mix: whatever is heard while it runs is in the file.
    Project,
    /// One track: its name and every region on it.
    Track {
        index: usize,
        name: String,
        regions: Vec<RegionContent>,
    },
    /// Some regions of one track, each with its index and what it held.
    Regions {
        track: usize,
        track_name: String,
        regions: Vec<(usize, RegionContent)>,
    },
}

impl Target {
    fn track(project: &Project, index: usize) -> Result<Self, String> {
        let t = project.tracks.get(index).ok_or("no such track")?;
        Ok(Self::Track {
            index,
            name: t.name.clone(),
            regions: t.regions.iter().map(RegionContent::of).collect(),
        })
    }

    fn regions(project: &Project, track: usize, regions: &[usize]) -> Result<Self, String> {
        let t = project.tracks.get(track).ok_or("no such track")?;
        let held = regions
            .iter()
            .map(|&r| {
                t.regions
                    .get(r)
                    .map(|reg| (r, RegionContent::of(reg)))
                    .ok_or("no such region")
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::Regions {
            track,
            track_name: t.name.clone(),
            regions: held,
        })
    }

    /// Whether the project still holds what this bounce was made from.
    pub fn still_there(&self, project: &Project) -> bool {
        match self {
            Self::Project => true,
            Self::Track {
                index,
                name,
                regions,
            } => project.tracks.get(*index).is_some_and(|t| {
                t.name == *name
                    && t.regions.len() == regions.len()
                    && t.regions.iter().zip(regions).all(|(r, c)| c.matches(r))
            }),
            Self::Regions {
                track,
                track_name,
                regions,
            } => project.tracks.get(*track).is_some_and(|t| {
                t.name == *track_name
                    && regions
                        .iter()
                        .all(|(i, content)| t.regions.get(*i).is_some_and(|r| content.matches(r)))
            }),
        }
    }
}

/// Target loudness configuration for audio normalization (ITU-R BS.1770 / EBU R128).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessTarget {
    /// Target integrated LUFS (e.g. -14.0 for Spotify/YouTube, -16.0 for Apple Music, -23.0 for EBU R128).
    pub lufs: f32,
    /// Maximum True-Peak ceiling in dBTP (-1.0 dBTP is standard to prevent inter-sample clipping).
    pub true_peak_ceiling_db: f32,
}

impl LoudnessTarget {
    /// Streaming standard for Spotify, YouTube, Tidal (-14.0 LUFS, -1.0 dBTP ceiling)
    pub const SPOTIFY: Self = Self {
        lufs: -14.0,
        true_peak_ceiling_db: -1.0,
    };
    /// Streaming standard for Apple Music and Podcasts (-16.0 LUFS, -1.0 dBTP ceiling)
    pub const APPLE_MUSIC: Self = Self {
        lufs: -16.0,
        true_peak_ceiling_db: -1.0,
    };
    /// European Television broadcast standard EBU R128 (-23.0 LUFS, -1.0 dBTP ceiling)
    pub const EBU_R128: Self = Self {
        lufs: -23.0,
        true_peak_ceiling_db: -1.0,
    };
    /// Club / DJ Electronic Master (-9.0 LUFS, -0.3 dBTP ceiling)
    pub const CLUB: Self = Self {
        lufs: -9.0,
        true_peak_ceiling_db: -0.3,
    };
}

impl Default for LoudnessTarget {
    fn default() -> Self {
        Self::SPOTIFY
    }
}

/// Outcome report of the loudness normalization process.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessReport {
    pub lufs: f32,
    pub true_peak_db: f32,
    pub gain_db: f32,
}

/// A finished bounce: the file, whether its region can take its place, and optional loudness report.
pub struct Finished {
    pub path: PathBuf,
    pub placeable: bool,
    pub loudness: Option<LoudnessReport>,
}

/// A bounce in progress.
pub struct Bounce {
    /// Gathers the engine's frames; returns them when stopped.
    collector: Option<JoinHandle<Vec<f32>>>,
    stop: Arc<AtomicBool>,
    /// Frames the engine's ring had no room for, shared with the engine.
    dropped: Arc<AtomicU64>,
    target: Target,
    pub start_beat: f64,
    pub end_beat: f64,
    /// The playhead's place, the cycle, the metronome and the solos before the bounce, put
    /// back when it ends.
    was_at: f64,
    was_cycle: bool,
    was_metronome: bool,
    /// Each track's name and solo state before a track bounce (empty for a project bounce).
    was_solos: Vec<(String, bool)>,
    path: PathBuf,
    sample_rate: u32,
    pub loudness: Option<LoudnessTarget>,
}

impl Bounce {
    /// Normalizes the bounce to a target loudness (ITU-R BS.1770 / EBU R128).
    pub fn with_loudness(mut self, target: LoudnessTarget) -> Self {
        self.loudness = Some(target);
        self
    }

    /// Starts bouncing `start_beat` to `end_beat` into `path`, and plays from there.
    pub fn start(
        engine: &mut Engine,
        project: &mut Project,
        start_beat: f64,
        end_beat: f64,
        path: PathBuf,
    ) -> Result<Self, String> {
        Self::begin(
            engine,
            project,
            start_beat,
            end_beat,
            path,
            Target::Project,
            None,
        )
    }

    /// Bounces one track: the other tracks are soloed off for the bounce, so only this track —
    /// with its effects, sends and their returns — reaches the master.
    pub fn start_track(
        engine: &mut Engine,
        project: &mut Project,
        track: usize,
        start_beat: f64,
        end_beat: f64,
        path: PathBuf,
    ) -> Result<Self, String> {
        let target = Target::track(project, track)?;
        Self::begin(
            engine,
            project,
            start_beat,
            end_beat,
            path,
            target,
            Some(track),
        )
    }

    /// Bounces `regions` of `track` over their span, soloing the track. The regions are only
    /// replaced when they are still the ones bounced (see [`Finished::placeable`]).
    pub fn start_regions(
        engine: &mut Engine,
        project: &mut Project,
        track: usize,
        regions: &[usize],
        start_beat: f64,
        end_beat: f64,
        path: PathBuf,
    ) -> Result<Self, String> {
        let target = Target::regions(project, track, regions)?;
        Self::begin(
            engine,
            project,
            start_beat,
            end_beat,
            path,
            target,
            Some(track),
        )
    }

    fn begin(
        engine: &mut Engine,
        project: &mut Project,
        start_beat: f64,
        end_beat: f64,
        path: PathBuf,
        target: Target,
        solo: Option<usize>,
    ) -> Result<Self, String> {
        let (frames, received) = RingBuffer::<[f32; 2]>::new(RING_FRAMES);
        let stop = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicU64::new(0));
        let collector = {
            let stop = stop.clone();
            thread::Builder::new()
                .name("prowave-bounce".into())
                .spawn(move || collect(received, &stop))
                .map_err(|e| format!("cannot start the bounce: {e}"))?
        };
        let sink = CaptureSink {
            frames,
            dropped: dropped.clone(),
        };
        if !engine.send(Command::SetCapture(Some(sink))) {
            stop.store(true, Ordering::Release);
            let _ = collector.join();
            return Err("the engine is busy; try the bounce again".to_string());
        }
        let was_solos = match solo {
            Some(track) => solo_only(&mut project.tracks, track),
            None => Vec::new(),
        };
        let bounce = Self {
            collector: Some(collector),
            stop,
            dropped,
            target,
            start_beat,
            end_beat,
            was_at: project.position_beats,
            was_cycle: project.cycle,
            was_metronome: project.metronome,
            was_solos,
            path,
            sample_rate: engine.sample_rate(),
            loudness: None,
        };
        // A bounce plays once through, from its start, without the metronome's clicks (they
        // are mixed before the master fader, so they would be in the file).
        bounce.take_transport(project);
        Ok(bounce)
    }

    /// Takes the transport for the bounce: no cycle, no metronome, playing from the start.
    fn take_transport(&self, project: &mut Project) {
        project.cycle = false;
        project.metronome = false;
        project.position_beats = self.start_beat;
        project.playing = true;
    }

    /// Puts the transport back the way it was before the bounce.
    fn restore_transport(&self, project: &mut Project) {
        project.playing = false;
        project.cycle = self.was_cycle;
        project.metronome = self.was_metronome;
        project.position_beats = self.was_at;
        restore_solos(&mut project.tracks, &self.was_solos);
    }

    /// Whether the bounce is still running: the transport plays and has not reached the end.
    pub fn poll(&self, project: &Project) -> bool {
        project.playing && project.position_beats < self.end_beat
    }

    /// The share of the section that has been played, 0 to 1.
    pub fn progress(&self, project: &Project) -> f32 {
        let span = self.end_beat - self.start_beat;
        if span <= 0.0 {
            return 1.0;
        }
        (((project.position_beats - self.start_beat) / span) as f32).clamp(0.0, 1.0)
    }

    /// Stops the bounce and puts the transport back. With `reached_end`, the frames gathered
    /// are written to the file; without it (the user stopped) nothing is written.
    ///
    /// Errors (no file is written): the engine did not take the stop command, the capture ring
    /// lost frames, or the track being bounced changed while it ran.
    pub fn finish(
        mut self,
        engine: &mut Engine,
        project: &mut Project,
        reached_end: bool,
    ) -> Result<Option<Finished>, String> {
        let stopped = engine.send(Command::SetCapture(None));
        self.restore_transport(project);
        self.stop.store(true, Ordering::Release);
        let mut samples = match self.collector.take().map(JoinHandle::join) {
            Some(Ok(samples)) => samples,
            Some(Err(_)) => return Err("the bounce stopped unexpectedly".to_string()),
            None => Vec::new(),
        };
        if !reached_end {
            return Ok(None);
        }
        if !stopped {
            return Err("the engine did not stop the bounce; no file was written".to_string());
        }
        let lost = self.dropped.load(Ordering::Relaxed);
        if lost > 0 {
            return Err(format!(
                "the bounce lost {lost} frames while the app was busy, so no file was written; bounce again"
            ));
        }
        let placeable = self.target.still_there(project);
        if matches!(self.target, Target::Track { .. }) && !placeable {
            return Err(
                "the track changed during the bounce, so no file was written; bounce again"
                    .to_string(),
            );
        }
        let loudness_report = self
            .loudness
            .map(|target| normalize_to_loudness(&mut samples, self.sample_rate, target));
        write_wav(&self.path, &samples, self.sample_rate)
            .map_err(|e| format!("{}: {e}", self.path.display()))?;
        Ok(Some(Finished {
            path: self.path.clone(),
            placeable,
            loudness: loudness_report,
        }))
    }
}

impl Drop for Bounce {
    /// A bounce that is dropped without `finish` still stops its collector thread.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.collector.take() {
            let _ = handle.join();
        }
    }
}

/// Gathers the frames the engine writes until `stop` is set (every frame written before the
/// stop is read: the stop flag is read before each drain).
fn collect(mut received: Consumer<[f32; 2]>, stop: &AtomicBool) -> Vec<f32> {
    let mut samples = Vec::new();
    loop {
        let stopping = stop.load(Ordering::Acquire);
        while let Ok(frame) = received.pop() {
            samples.push(frame[0]);
            samples.push(frame[1]);
        }
        if stopping {
            return samples;
        }
        thread::sleep(COLLECT_EVERY);
    }
}

/// Measures and applies loudness normalization conforming to ITU-R BS.1770 / EBU R128.
pub fn normalize_to_loudness(
    samples: &mut [f32],
    sample_rate: u32,
    target: LoudnessTarget,
) -> LoudnessReport {
    if samples.is_empty() {
        return LoudnessReport {
            lufs: -70.0,
            true_peak_db: -70.0,
            gain_db: 0.0,
        };
    }
    let frames = samples.len() / 2;
    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);
    for chunk in samples.chunks_exact(2) {
        left.push(chunk[0]);
        right.push(chunk[1]);
    }
    let planar = vec![left, right];
    let mut meter = nova_dsp::meter::LoudnessMeter::new(sample_rate as f32, 2);
    meter.process(&planar, frames);

    let lufs = meter.integrated_lufs();
    let true_peak = meter.true_peak_db();

    // If audio is silent or too quiet to measure (below -70 LUFS):
    if lufs <= -70.0 {
        if true_peak > target.true_peak_ceiling_db {
            let gain_db = target.true_peak_ceiling_db - true_peak;
            let gain = 10.0f32.powf(gain_db / 20.0);
            for s in samples.iter_mut() {
                *s *= gain;
            }
            return LoudnessReport {
                lufs,
                true_peak_db: target.true_peak_ceiling_db,
                gain_db,
            };
        }
        return LoudnessReport {
            lufs,
            true_peak_db: true_peak,
            gain_db: 0.0,
        };
    }

    let mut gain_db = target.lufs - lufs;
    // Enforce hard ceiling on True Peak to avoid clipping
    if true_peak + gain_db > target.true_peak_ceiling_db {
        gain_db = target.true_peak_ceiling_db - true_peak;
    }

    let gain = 10.0f32.powf(gain_db / 20.0);
    for s in samples.iter_mut() {
        *s *= gain;
    }

    LoudnessReport {
        lufs: lufs + gain_db,
        true_peak_db: true_peak + gain_db,
        gain_db,
    }
}

/// Solos `track` and mutes the solo of every other track. Returns each track's name and solo
/// state before, for [`restore_solos`].
pub fn solo_only(tracks: &mut [Track], track: usize) -> Vec<(String, bool)> {
    let before = tracks.iter().map(|t| (t.name.clone(), t.solo)).collect();
    for (i, t) in tracks.iter_mut().enumerate() {
        t.solo = i == track;
    }
    before
}

/// Puts back the solo states from [`solo_only`]. Each saved state goes to the track at the
/// same position if it still has the same name, otherwise to the first track with that name;
/// a track that was deleted meanwhile is skipped.
pub fn restore_solos(tracks: &mut [Track], saved: &[(String, bool)]) {
    for (position, (name, solo)) in saved.iter().enumerate() {
        let by_position = tracks.get(position).filter(|t| t.name == *name).is_some();
        let index = if by_position {
            Some(position)
        } else {
            tracks.iter().position(|t| t.name == *name)
        };
        if let Some(t) = index.and_then(|i| tracks.get_mut(i)) {
            t.solo = *solo;
        }
    }
}

/// `dir/stem.wav`, or `dir/stem 2.wav`, `stem 3.wav`… whichever is free. A bounce never
/// overwrites a file an earlier bounce wrote, because undo can still point at it.
pub fn free_path(dir: &Path, stem: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.wav"));
    if !first.exists() {
        return first;
    }
    let mut n = 2u32;
    loop {
        let path = dir.join(format!("{stem} {n}.wav"));
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TrackKind;

    /// A bounce with no engine behind it: its state maths is what the tests check.
    fn bounced(start_beat: f64, end_beat: f64) -> Bounce {
        Bounce {
            collector: None,
            stop: Arc::new(AtomicBool::new(false)),
            dropped: Arc::new(AtomicU64::new(0)),
            target: Target::Project,
            start_beat,
            end_beat,
            was_at: 0.0,
            was_cycle: false,
            was_metronome: false,
            was_solos: Vec::new(),
            path: PathBuf::from("bounce.wav"),
            sample_rate: 48_000,
            loudness: None,
        }
    }

    fn region(name: &str, start_bar: f32, notes: Vec<Note>) -> Region {
        Region {
            name: name.into(),
            start_bar,
            len_bars: 2.0,
            notes,
            controls: Vec::new(),
            folder: None,
            audio: None,
            params: RegionParams::default(),
            generated: None,
        }
    }

    fn note(pitch: u8) -> Note {
        Note {
            start: 0.0,
            len: 1.0,
            pitch,
            vel: 100,
        }
    }

    /// A project with one instrument track holding `regions`.
    fn project_with(regions: Vec<Region>) -> Project {
        let mut p = Project::empty();
        p.tracks.clear();
        p.add_track(TrackKind::Instrument);
        p.tracks[0].regions = regions;
        p
    }

    #[test]
    fn a_bounce_runs_from_its_start_to_its_end_beat() {
        let running = bounced(4.0, 12.0);
        let mut project = Project::empty();
        project.playing = true;
        project.position_beats = 4.0;
        assert!(running.poll(&project), "just started");
        assert_eq!(running.progress(&project), 0.0);
        project.position_beats = 8.0;
        assert!(running.poll(&project));
        assert!((running.progress(&project) - 0.5).abs() < 1e-6);
        project.position_beats = 12.0;
        assert!(!running.poll(&project), "the end beat ends it");
        assert_eq!(running.progress(&project), 1.0);
    }

    #[test]
    fn stopping_the_transport_ends_a_bounce() {
        let running = bounced(0.0, 16.0);
        let mut project = Project::empty();
        project.position_beats = 3.0;
        project.playing = false;
        assert!(!running.poll(&project));
        assert!((running.progress(&project) - 3.0 / 16.0).abs() < 1e-6);
    }

    #[test]
    fn a_bounce_silences_the_metronome_and_gives_the_transport_back() {
        let mut running = bounced(4.0, 12.0);
        running.was_at = 30.0;
        running.was_cycle = true;
        running.was_metronome = true;
        let mut project = Project::empty();
        project.cycle = true;
        project.metronome = true;
        running.take_transport(&mut project);
        assert!(!project.cycle && !project.metronome, "the clicks stay out");
        assert!(project.playing);
        assert_eq!(project.position_beats, 4.0);
        running.restore_transport(&mut project);
        assert!(project.cycle && project.metronome);
        assert!(!project.playing);
        assert_eq!(project.position_beats, 30.0);
    }

    #[test]
    fn solo_only_solos_one_track_and_restore_gives_the_solos_back() {
        let mut project = Project::empty();
        project.tracks.clear();
        project.add_track(TrackKind::Instrument);
        project.add_track(TrackKind::Instrument);
        project.tracks[0].solo = false;
        project.tracks[1].solo = true;
        let saved = solo_only(&mut project.tracks, 0);
        assert!(project.tracks[0].solo && !project.tracks[1].solo);
        restore_solos(&mut project.tracks, &saved);
        assert!(!project.tracks[0].solo && project.tracks[1].solo);
    }

    #[test]
    fn restored_solos_follow_a_track_that_moved() {
        let mut project = Project::empty();
        project.tracks.clear();
        project.add_track(TrackKind::Instrument);
        project.add_track(TrackKind::Instrument);
        let names: Vec<String> = project.tracks.iter().map(|t| t.name.clone()).collect();
        project.tracks[1].solo = true;
        let saved = solo_only(&mut project.tracks, 0);
        // The second track is moved to the front while the bounce runs.
        project.tracks.swap(0, 1);
        restore_solos(&mut project.tracks, &saved);
        let solo_name = project
            .tracks
            .iter()
            .find(|t| t.solo)
            .map(|t| t.name.clone());
        assert_eq!(
            solo_name,
            Some(names[1].clone()),
            "the solo stays with its track"
        );
        assert_eq!(project.tracks.iter().filter(|t| t.solo).count(), 1);
    }

    #[test]
    fn a_region_that_was_not_changed_is_still_there() {
        let project = project_with(vec![region("Lead", 1.0, vec![note(60)])]);
        let target = Target::regions(&project, 0, &[0]).unwrap();
        assert!(target.still_there(&project));
    }

    #[test]
    fn a_region_moved_deleted_or_edited_is_not_still_there() {
        let mut project = project_with(vec![
            region("Lead", 1.0, vec![note(60)]),
            region("Bass", 3.0, vec![note(36)]),
        ]);
        let target = Target::regions(&project, 0, &[0]).unwrap();

        // An edited note.
        project.tracks[0].regions[0].notes[0].pitch = 61;
        assert!(!target.still_there(&project), "the notes changed");
        project.tracks[0].regions[0].notes[0].pitch = 60;
        assert!(target.still_there(&project), "changed back, same again");

        // A moved region.
        project.tracks[0].regions[0].start_bar = 2.0;
        assert!(!target.still_there(&project), "moved");
        project.tracks[0].regions[0].start_bar = 1.0;

        // The region at that index is another one (the first was deleted).
        project.tracks[0].regions.remove(0);
        assert!(
            !target.still_there(&project),
            "another region now sits there"
        );
    }

    #[test]
    fn a_track_renamed_or_reordered_is_not_still_there() {
        let mut project = project_with(vec![region("Lead", 1.0, vec![note(60)])]);
        project.add_track(TrackKind::Instrument);
        let target = Target::regions(&project, 0, &[0]).unwrap();
        project.tracks.swap(0, 1);
        assert!(!target.still_there(&project), "the track moved");
        project.tracks.swap(0, 1);
        project.tracks[0].name = "Renamed".into();
        assert!(!target.still_there(&project), "the track was renamed");
    }

    #[test]
    fn a_track_target_notices_a_region_added_or_removed() {
        let mut project = project_with(vec![region("Lead", 1.0, vec![note(60)])]);
        let target = Target::track(&project, 0).unwrap();
        assert!(target.still_there(&project));
        project.tracks[0]
            .regions
            .push(region("Extra", 5.0, vec![note(62)]));
        assert!(!target.still_there(&project));
    }

    #[test]
    fn a_track_target_that_no_longer_exists_is_not_still_there() {
        let mut project = project_with(vec![region("Lead", 1.0, vec![note(60)])]);
        let target = Target::track(&project, 0).unwrap();
        project.tracks.clear();
        assert!(!target.still_there(&project));
    }

    #[test]
    fn free_path_never_returns_an_existing_file() {
        let dir = std::env::temp_dir().join(format!("prowave-free-path-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = free_path(&dir, "Lead 1.000");
        assert_eq!(first, dir.join("Lead 1.000.wav"));
        std::fs::write(&first, b"x").unwrap();
        let second = free_path(&dir, "Lead 1.000");
        assert_eq!(second, dir.join("Lead 1.000 2.wav"));
        std::fs::write(&second, b"x").unwrap();
        assert_eq!(free_path(&dir, "Lead 1.000"), dir.join("Lead 1.000 3.wav"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_collector_keeps_every_frame_written_while_it_runs() {
        let (mut frames, received) = RingBuffer::<[f32; 2]>::new(64);
        let stop = Arc::new(AtomicBool::new(false));
        let collector = {
            let stop = stop.clone();
            thread::spawn(move || collect(received, &stop))
        };
        // Far more frames than the ring holds, so the collector must keep emptying it.
        for i in 0..20_000 {
            let frame = [i as f32, -(i as f32)];
            while frames.push(frame).is_err() {
                thread::yield_now();
            }
        }
        stop.store(true, Ordering::Release);
        let samples = collector.join().unwrap();
        assert_eq!(samples.len(), 20_000 * 2);
        assert_eq!(samples[2 * 19_999], 19_999.0);
        assert_eq!(samples[2 * 19_999 + 1], -19_999.0);
    }

    #[test]
    fn loudness_normalization_reaches_target_and_respects_ceiling() {
        let sr = 48_000;
        let frames = sr * 2; // 2 seconds
        // Generate a 1 kHz sine wave at roughly -20 dBFS
        let mut samples = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let s = (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin() * 0.1;
            samples.push(s);
            samples.push(s);
        }

        let target = LoudnessTarget::SPOTIFY; // -14 LUFS, -1.0 dBTP
        let report = normalize_to_loudness(&mut samples, sr as u32, target);

        // Should apply positive gain to reach target
        assert!(report.gain_db > 0.0, "gain should be positive: {}", report.gain_db);
        assert!(
            report.true_peak_db <= target.true_peak_ceiling_db + 0.1,
            "true peak {} should be <= ceiling {}",
            report.true_peak_db,
            target.true_peak_ceiling_db
        );
        // Verify audio was actually scaled
        let max_sample = samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(max_sample > 0.1, "samples should be boosted");
    }
}

