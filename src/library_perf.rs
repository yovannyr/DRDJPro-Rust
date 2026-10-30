//! Opt-in control-plane measurements, ending before display presentation.
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::library::SearchSpec;

pub const MIN_TRACKS: usize = 3_000;
pub const MIN_ACTIONS: usize = 10;
pub const MIN_LOAD_TIME: Duration = Duration::from_secs(120);
const MAX_SAMPLES: usize = 4_096;
const DISPATCH_BUDGET: Duration = Duration::from_millis(50);
const RENDER_P95_BUDGET: Duration = Duration::from_millis(250);
const RENDER_MAX_BUDGET: Duration = Duration::from_millis(1_000);
const TICK_GAP_BUDGET: Duration = Duration::from_millis(100);
const BUS_FRESHNESS: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Search,
    Sort,
    Page,
}

impl Operation {
    const ALL: [Self; 3] = [Self::Search, Self::Sort, Self::Page];
}

struct Action {
    operation: Operation,
    started: Instant,
    spec: Option<SearchSpec>,
    dispatch: Option<Duration>,
    under_load: bool,
}

struct Ready {
    action: Action,
    model: Duration,
}

struct Sample {
    operation: Operation,
    dispatch: Duration,
    model: Duration,
    render: Duration,
    under_load: bool,
}

pub struct Recorder {
    started: Instant,
    eligible_load: bool,
    bus_live: bool,
    last_bus_frame: Option<u64>,
    last_bus_progress: Option<Instant>,
    render_stalls: usize,
    last_tick: Option<(Instant, bool)>,
    load_time: Duration,
    max_tick_gap: Option<Duration>,
    max_tracks: usize,
    active: Option<Action>,
    ready: Option<Ready>,
    samples: Vec<Sample>,
    superseded: usize,
    errors: usize,
    overflow: usize,
}

impl Recorder {
    pub fn new(now: Instant) -> Self {
        Self {
            started: now,
            eligible_load: false,
            bus_live: false,
            last_bus_frame: None,
            last_bus_progress: None,
            render_stalls: 0,
            last_tick: None,
            load_time: Duration::ZERO,
            max_tick_gap: None,
            max_tracks: 0,
            active: None,
            ready: None,
            samples: Vec::with_capacity(MAX_SAMPLES),
            superseded: 0,
            errors: 0,
            overflow: 0,
        }
    }

    pub fn context(&mut self, tracks: usize, both_playing: bool) {
        self.max_tracks = self.max_tracks.max(tracks);
        self.eligible_load = both_playing && tracks >= MIN_TRACKS && self.bus_live;
        if !self.eligible_load {
            if let Some((_, eligible)) = self.last_tick.as_mut() {
                *eligible = false;
            }
        }
        if let Some(action) = self.active.as_mut() {
            action.under_load &= self.eligible_load;
        }
        if let Some(ready) = self.ready.as_mut() {
            ready.action.under_load &= self.eligible_load;
        }
    }

    pub fn tick(&mut self, now: Instant, tracks: usize, both_playing: bool, bus_frame: u64) {
        let was_live = self.bus_live;
        if self
            .last_bus_frame
            .is_some_and(|previous| bus_frame > previous)
        {
            self.last_bus_progress = Some(now);
        } else if self
            .last_bus_frame
            .is_some_and(|previous| bus_frame < previous)
        {
            self.last_bus_progress = None;
        }
        self.last_bus_frame = Some(bus_frame);
        self.bus_live = self
            .last_bus_progress
            .is_some_and(|last| now.duration_since(last) <= BUS_FRESHNESS);
        if was_live && !self.bus_live && both_playing && tracks >= MIN_TRACKS {
            self.render_stalls += 1;
        }
        self.context(tracks, both_playing);
        if let Some((previous, true)) = self.last_tick {
            if self.eligible_load {
                let gap = now.duration_since(previous);
                self.load_time += gap;
                self.max_tick_gap = Some(self.max_tick_gap.map_or(gap, |old| old.max(gap)));
            }
        }
        self.last_tick = Some((now, self.eligible_load));
    }

    pub fn begin(&mut self, operation: Operation, now: Instant) {
        if self.active.is_some() || self.ready.is_some() {
            self.superseded += 1;
        }
        self.ready = None;
        self.active = Some(Action {
            operation,
            started: now,
            spec: None,
            dispatch: None,
            under_load: self.eligible_load,
        });
    }

    pub fn bind(&mut self, spec: &SearchSpec) {
        let bound = self
            .active
            .as_ref()
            .or_else(|| self.ready.as_ref().map(|ready| &ready.action))
            .and_then(|action| action.spec.as_ref());
        if bound.is_some_and(|bound| bound != spec) {
            self.active = None;
            self.ready = None;
            self.superseded += 1;
        } else if let Some(action) = self.active.as_mut() {
            action.spec = Some(spec.clone());
        }
    }

    pub fn dispatched(&mut self, now: Instant) {
        if let Some(action) = self
            .active
            .as_mut()
            .or_else(|| self.ready.as_mut().map(|ready| &mut ready.action))
        {
            action.dispatch = Some(now.duration_since(action.started));
        }
    }

    pub fn model_ready(&mut self, spec: &SearchSpec, now: Instant) {
        if self.active.as_ref().and_then(|action| action.spec.as_ref()) == Some(spec) {
            let action = self.active.take().unwrap();
            let model = now.duration_since(action.started);
            self.ready = Some(Ready { action, model });
        }
    }

    pub fn rendered(&mut self, now: Instant) {
        let Some(ready) = self.ready.take() else {
            return;
        };
        let Some(dispatch) = ready.action.dispatch else {
            self.errors += 1;
            return;
        };
        if self.samples.len() == MAX_SAMPLES {
            self.overflow += 1;
            return;
        }
        self.samples.push(Sample {
            operation: ready.action.operation,
            dispatch,
            model: ready.model,
            render: now.duration_since(ready.action.started),
            under_load: ready.action.under_load,
        });
    }

    pub fn failed(&mut self) {
        self.active = None;
        self.ready = None;
        self.errors += 1;
    }

    fn count(&self, operation: Operation) -> usize {
        self.samples
            .iter()
            .filter(|sample| sample.under_load && sample.operation == operation)
            .count()
    }

    pub fn progress(&self) -> String {
        format!(
            "PERF {}/120s Q{}/10 S{}/10 P{}/10",
            self.load_time.as_secs(),
            self.count(Operation::Search),
            self.count(Operation::Sort),
            self.count(Operation::Page)
        )
    }

    pub fn report(&self, now: Instant) -> Report {
        let operations: Vec<_> = Operation::ALL
            .into_iter()
            .map(|operation| {
                let samples: Vec<_> = self
                    .samples
                    .iter()
                    .filter(|sample| sample.under_load && sample.operation == operation)
                    .collect();
                OperationReport {
                    operation,
                    samples: samples.len(),
                    handler: Stats::new(samples.iter().map(|sample| sample.dispatch)),
                    model: Stats::new(samples.iter().map(|sample| sample.model)),
                    after_rendering: Stats::new(samples.iter().map(|sample| sample.render)),
                }
            })
            .collect();
        let unfinished = usize::from(self.active.is_some() || self.ready.is_some());
        let coverage_complete = self.load_time >= MIN_LOAD_TIME
            && operations.iter().all(|op| op.samples >= MIN_ACTIONS)
            && unfinished == 0
            && self.overflow == 0;
        let budgets_met = self.render_stalls == 0
            && self.max_tick_gap.is_some_and(|gap| gap <= TICK_GAP_BUDGET)
            && Operation::ALL.into_iter().all(|operation| {
                let samples = || {
                    self.samples
                        .iter()
                        .filter(|sample| sample.under_load && sample.operation == operation)
                };
                let mut render: Vec<_> = samples().map(|sample| sample.render).collect();
                render.sort_unstable();
                !render.is_empty()
                    && percentile95(&render) <= RENDER_P95_BUDGET
                    && render.last().is_some_and(|max| *max <= RENDER_MAX_BUDGET)
                    && samples().all(|sample| sample.dispatch <= DISPATCH_BUDGET)
            });
        let outcome = if self.errors > 0 {
            "measurement_failed"
        } else if !coverage_complete {
            "insufficient_coverage"
        } else if !budgets_met {
            "budget_exceeded"
        } else {
            "within_budgets"
        };
        Report {
            schema_version: 1,
            measurement: "controller_handler_to_model_and_after_rendering_before_presentation",
            scope: "sampled dual-deck playback and bus progress; no physical input/display latency or audio-dropout measurement",
            platform: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            outcome,
            coverage_complete,
            budgets_met,
            elapsed_seconds: now.duration_since(self.started).as_secs_f64(),
            sampled_load_seconds: self.load_time.as_secs_f64(),
            max_scope_tracks: self.max_tracks,
            max_tick_gap_ms: self.max_tick_gap.map(milliseconds),
            thresholds: Thresholds {
                min_scope_tracks: MIN_TRACKS,
                min_sampled_load_seconds: MIN_LOAD_TIME.as_secs(),
                min_samples_per_operation: MIN_ACTIONS,
                handler_max_ms: milliseconds(DISPATCH_BUDGET),
                after_rendering_p95_ms: milliseconds(RENDER_P95_BUDGET),
                after_rendering_max_ms: milliseconds(RENDER_MAX_BUDGET),
                tick_gap_max_ms: milliseconds(TICK_GAP_BUDGET),
                bus_progress_freshness_ms: milliseconds(BUS_FRESHNESS),
            },
            operations,
            samples: self
                .samples
                .iter()
                .map(|sample| SampleReport {
                    operation: sample.operation,
                    under_load: sample.under_load,
                    handler_ms: milliseconds(sample.dispatch),
                    model_ms: milliseconds(sample.model),
                    after_rendering_ms: milliseconds(sample.render),
                })
                .collect(),
            superseded_actions: self.superseded,
            measurement_errors: self.errors,
            unfinished_actions: unfinished,
            discarded_samples: self.overflow,
            observed_render_stalls: self.render_stalls,
        }
    }
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn percentile95(sorted: &[Duration]) -> Duration {
    sorted[(sorted.len() * 95).div_ceil(100) - 1]
}

#[derive(Serialize)]
pub struct Stats {
    min_ms: f64,
    p95_ms: f64,
    max_ms: f64,
}

impl Stats {
    fn new(values: impl Iterator<Item = Duration>) -> Option<Self> {
        let mut values: Vec<_> = values.collect();
        if values.is_empty() {
            return None;
        }
        values.sort_unstable();
        Some(Self {
            min_ms: milliseconds(values[0]),
            p95_ms: milliseconds(percentile95(&values)),
            max_ms: milliseconds(*values.last().unwrap()),
        })
    }
}

#[derive(Serialize)]
pub struct OperationReport {
    operation: Operation,
    samples: usize,
    handler: Option<Stats>,
    model: Option<Stats>,
    after_rendering: Option<Stats>,
}

#[derive(Serialize)]
pub struct SampleReport {
    operation: Operation,
    under_load: bool,
    handler_ms: f64,
    model_ms: f64,
    after_rendering_ms: f64,
}

#[derive(Serialize)]
pub struct Thresholds {
    min_scope_tracks: usize,
    min_sampled_load_seconds: u64,
    min_samples_per_operation: usize,
    handler_max_ms: f64,
    after_rendering_p95_ms: f64,
    after_rendering_max_ms: f64,
    tick_gap_max_ms: f64,
    bus_progress_freshness_ms: f64,
}

#[derive(Serialize)]
pub struct Report {
    schema_version: u32,
    measurement: &'static str,
    scope: &'static str,
    platform: &'static str,
    architecture: &'static str,
    pub outcome: &'static str,
    coverage_complete: bool,
    budgets_met: bool,
    elapsed_seconds: f64,
    sampled_load_seconds: f64,
    max_scope_tracks: usize,
    max_tick_gap_ms: Option<f64>,
    thresholds: Thresholds,
    operations: Vec<OperationReport>,
    samples: Vec<SampleReport>,
    superseded_actions: usize,
    measurement_errors: usize,
    unfinished_actions: usize,
    discarded_samples: usize,
    observed_render_stalls: usize,
}

pub struct ReportFile {
    file: File,
    pub path: PathBuf,
}

impl ReportFile {
    pub fn from_env() -> Result<Option<Self>, String> {
        std::env::var_os("DRDJ_LIBRARY_PERF_PATH")
            .map(|path| Self::open(Path::new(&path), &crate::library_store::default_path()?))
            .transpose()
    }

    fn open(path: &Path, library_path: &Path) -> Result<Self, String> {
        if !path.is_absolute() {
            return Err("DRDJ_LIBRARY_PERF_PATH must be an absolute report path".into());
        }
        let report_parent = path
            .parent()
            .ok_or("Performance report has no parent directory")?
            .canonicalize()
            .map_err(|error| format!("Cannot resolve performance report directory: {error}"))?;
        let library_parent = library_path
            .parent()
            .ok_or("Library has no parent directory")?
            .canonicalize()
            .map_err(|error| {
                format!("Cannot resolve library directory for report safety: {error}")
            })?;
        if crate::library::identity(&library_parent) == crate::library::identity(&report_parent) {
            return Err("Store the performance report outside the library directory to protect migration, backup and lock files".into());
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| format!("Cannot create library performance report: {error}"))?;
        Ok(Self {
            file,
            path: path.to_owned(),
        })
    }

    pub fn finish(mut self, report: &Report) -> Result<(), String> {
        serde_json::to_writer_pretty(&mut self.file, report)
            .map_err(|error| format!("Cannot write library performance report: {error}"))?;
        self.file
            .write_all(b"\n")
            .and_then(|()| self.file.flush())
            .and_then(|()| self.file.sync_all())
            .map_err(|error| format!("Cannot persist library performance report: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> SearchSpec {
        crate::library::Library::default().search_spec(0)
    }

    fn complete(recorder: &mut Recorder, operation: Operation, now: Instant, render: Duration) {
        let spec = spec();
        recorder.begin(operation, now);
        recorder.bind(&spec);
        recorder.dispatched(now + Duration::from_millis(10));
        recorder.model_ready(&spec, now + Duration::from_millis(20));
        recorder.rendered(now + render);
    }

    fn covered() -> (Recorder, Instant) {
        let start = Instant::now();
        let mut recorder = Recorder::new(start);
        for i in 0..=1_201 {
            recorder.tick(
                start + Duration::from_millis(i * 100),
                MIN_TRACKS,
                true,
                i * 4_800,
            );
        }
        for operation in Operation::ALL {
            for i in 0..MIN_ACTIONS {
                complete(
                    &mut recorder,
                    operation,
                    start + Duration::from_secs(121 + i as u64),
                    RENDER_P95_BUDGET,
                );
            }
        }
        (recorder, start + Duration::from_secs(140))
    }

    #[test]
    fn exact_budgets_and_nearest_rank_p95_determine_outcome() {
        let (mut recorder, now) = covered();
        for sample in &mut recorder.samples {
            sample.dispatch = DISPATCH_BUDGET;
        }
        assert_eq!(recorder.report(now).outcome, "within_budgets");
        recorder.samples[0].dispatch += Duration::from_nanos(1);
        assert_eq!(recorder.report(now).outcome, "budget_exceeded");
        recorder.samples[0].dispatch = DISPATCH_BUDGET;
        recorder.max_tick_gap = Some(TICK_GAP_BUDGET + Duration::from_nanos(1));
        assert_eq!(recorder.report(now).outcome, "budget_exceeded");
        recorder.max_tick_gap = Some(TICK_GAP_BUDGET);
        recorder.samples[0].render = RENDER_P95_BUDGET + Duration::from_nanos(1);
        assert_eq!(recorder.report(now).outcome, "budget_exceeded");
        let values: Vec<_> = (1..=20).map(Duration::from_millis).collect();
        assert_eq!(percentile95(&values), Duration::from_millis(19));
        assert!(Stats::new(std::iter::empty()).is_none());
        let (mut recorder, now) = covered();
        for _ in 0..10 {
            complete(&mut recorder, Operation::Search, now, RENDER_P95_BUDGET);
        }
        recorder.samples[0].render = RENDER_MAX_BUDGET;
        assert_eq!(recorder.report(now).outcome, "within_budgets");
        recorder.samples[0].render += Duration::from_nanos(1);
        assert_eq!(recorder.report(now).outcome, "budget_exceeded");
    }

    #[test]
    fn missing_load_render_and_samples_never_pass() {
        let start = Instant::now();
        let mut recorder = Recorder::new(start);
        recorder.tick(start, MIN_TRACKS - 1, true, 0);
        recorder.tick(start + MIN_LOAD_TIME, MIN_TRACKS - 1, true, 48_000);
        complete(
            &mut recorder,
            Operation::Search,
            start,
            Duration::from_millis(30),
        );
        assert_eq!(recorder.load_time, Duration::ZERO);
        assert_eq!(recorder.count(Operation::Search), 0);
        assert_eq!(
            recorder.report(start + MIN_LOAD_TIME).outcome,
            "insufficient_coverage"
        );
        let (mut recorder, now) = covered();
        recorder.begin(Operation::Search, now);
        recorder.bind(&spec());
        recorder.dispatched(now);
        recorder.model_ready(&spec(), now);
        assert_eq!(recorder.report(now).unfinished_actions, 1);
        assert_eq!(recorder.report(now).outcome, "insufficient_coverage");
        recorder.context(MIN_TRACKS, false);
        recorder.rendered(now);
        assert!(!recorder.samples.last().unwrap().under_load);
        assert_eq!(recorder.report(now).outcome, "within_budgets");
    }

    #[test]
    fn superseded_and_stale_results_do_not_complete_the_wrong_action() {
        let start = Instant::now();
        let mut recorder = Recorder::new(start);
        recorder.tick(start, MIN_TRACKS, true, 0);
        recorder.tick(start + Duration::from_millis(1), MIN_TRACKS, true, 48);
        recorder.context(MIN_TRACKS, true);
        let first = spec();
        let mut second = first.clone();
        second.query = "private query must not appear in reports".into();
        recorder.begin(Operation::Search, start);
        recorder.bind(&first);
        recorder.begin(Operation::Sort, start);
        recorder.bind(&second);
        recorder.dispatched(start);
        recorder.model_ready(&first, start);
        recorder.rendered(start);
        assert!(recorder.samples.is_empty());
        recorder.model_ready(&second, start);
        recorder.rendered(start);
        assert_eq!(recorder.samples[0].operation, Operation::Sort);
        assert_eq!(recorder.superseded, 1);
        let json = serde_json::to_string(&recorder.report(start)).unwrap();
        assert!(!json.contains("private query"));
        recorder.begin(Operation::Search, start);
        recorder.bind(&first);
        recorder.bind(&second);
        assert!(recorder.active.is_none());
        assert_eq!(recorder.superseded, 2);
        recorder.failed();
        assert_eq!(recorder.report(start).outcome, "measurement_failed");
    }

    #[test]
    fn playing_flags_without_bus_progress_do_not_establish_load() {
        let now = Instant::now();
        let mut recorder = Recorder::new(now);
        for i in 0..=1_201 {
            recorder.tick(now + Duration::from_millis(i * 100), MIN_TRACKS, true, 0);
        }
        assert_eq!(recorder.load_time, Duration::ZERO);
        assert!(!recorder.eligible_load);
        recorder.tick(now + Duration::from_secs(121), MIN_TRACKS, true, 48_000);
        assert!(recorder.eligible_load);
        recorder.tick(
            now + Duration::from_secs(121) + BUS_FRESHNESS,
            MIN_TRACKS,
            true,
            48_000,
        );
        assert!(recorder.eligible_load);
        recorder.tick(
            now + Duration::from_secs(121) + BUS_FRESHNESS + Duration::from_nanos(1),
            MIN_TRACKS,
            true,
            48_000,
        );
        assert!(!recorder.eligible_load);
        assert_eq!(recorder.render_stalls, 1);
    }

    #[test]
    fn capacity_and_file_errors_are_explicit_and_never_qualified() {
        let (mut recorder, now) = covered();
        for _ in recorder.samples.len()..=MAX_SAMPLES {
            complete(
                &mut recorder,
                Operation::Page,
                now,
                Duration::from_millis(30),
            );
        }
        assert_eq!(recorder.samples.len(), MAX_SAMPLES);
        assert_eq!(recorder.overflow, 1);
        assert_eq!(recorder.report(now).outcome, "insufficient_coverage");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.json");
        let library = dir.path().join("catalog").join("library.sqlite3");
        std::fs::create_dir(library.parent().unwrap()).unwrap();
        let file = ReportFile::open(&path, &library).unwrap();
        assert!(ReportFile::open(&path, &library).is_err());
        file.finish(&recorder.report(now)).unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["discarded_samples"], 1);
        assert_eq!(json["operations"].as_array().unwrap().len(), 3);
        assert!(ReportFile::open(Path::new("relative.json"), &library).is_err());
        assert!(ReportFile::open(
            &dir.path().join("another-report.json"),
            &dir.path().join("missing").join("library.sqlite3")
        )
        .is_err());
        let readonly = ReportFile {
            file: File::open(&path).unwrap(),
            path,
        };
        assert!(readonly.finish(&recorder.report(now)).is_err());
    }

    #[test]
    fn report_cannot_claim_a_library_migration_backup_or_lock_location() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library.sqlite3");
        for name in [
            "library.json",
            "library.sqlite3",
            "library.sqlite3.bak",
            "library.sqlite3.lock",
            "other-report.json",
        ] {
            let report = dir.path().join(name);
            assert!(ReportFile::open(&report, &library).is_err());
            assert!(!report.exists());
        }
        let reports = dir.path().join("reports");
        std::fs::create_dir(&reports).unwrap();
        assert!(ReportFile::open(&reports.join("run.json"), &library).is_ok());
    }
}
