//! Feed ffmpeg's mono 22,050 Hz f32le stdout to this offline probe.
use std::{env, io, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env::args_os().nth(1).ok_or("expected model directory")?);
    let runtime = PathBuf::from(env::var_os("DRDJ_ORT_LIBRARY").ok_or("set DRDJ_ORT_LIBRARY")?);
    let mut tracker = drdj_ml::beat_tracking::Tracker::load(&directory, &runtime)?;
    let events = tracker.track(io::stdin().lock())?;
    println!(
        "{}",
        serde_json::json!({"beats_ms":events.beats_ms,"downbeats_ms":events.downbeats_ms})
    );
    Ok(())
}
