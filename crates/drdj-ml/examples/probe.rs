//! Explicit A0 benchmark, never part of application startup or normal tests.
use drdj_ml::{load_backend, BackendKind, Threads};
use std::{
    env,
    error::Error,
    io::{self, Write},
    path::PathBuf,
    time::Instant,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: probe ort|tract model.onnx threads".into());
    }
    let kind = match args[1].as_str() {
        "ort" => BackendKind::Ort,
        "tract" => BackendKind::Tract,
        _ => return Err("unknown backend".into()),
    };
    let model = PathBuf::from(&args[2]);
    let threads = Threads::new(args[3].parse()?)?;
    let runtime = env::var_os("DRDJ_ORT_LIBRARY").map(PathBuf::from);
    let start = Instant::now();
    let first = load_backend(kind, &model, threads, runtime.as_deref())?;
    let first_load_ms = start.elapsed().as_secs_f64() * 1000.0;
    drop(first);
    let start = Instant::now();
    let mut backend = load_backend(kind, &model, threads, runtime.as_deref())?;
    let warm_load_ms = start.elapsed().as_secs_f64() * 1000.0;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../tests/reference.json"))?;
    let mut max_error = 0.0_f64;
    for case in reference["cases"].as_array().ok_or("missing cases")? {
        let input: Vec<_> = case["input"]
            .as_array()
            .ok_or("missing input")?
            .iter()
            .map(|x| x.as_f64().unwrap_or(f64::NAN) as f32)
            .collect();
        let output = backend.infer(&input)?;
        let expected = case["output"].as_array().ok_or("missing output")?;
        if output.len() != expected.len() {
            return Err("output length differs".into());
        }
        for (actual, expected) in output.iter().zip(expected) {
            max_error =
                max_error.max((f64::from(*actual) - expected.as_f64().ok_or("bad output")?).abs());
        }
    }
    if max_error > 1e-5 {
        return Err("scalar parity failed".into());
    }
    let input = [0.0, 0.25, -0.5, 1.0, -1.0, 0.75, 0.125, -0.25];
    // 75 independent 8-frame chunks at an artificial 10Hz = synthetic minute.
    // Repeated ten times for timing, not a real audio/beat model throughput claim.
    let mut batches = Vec::with_capacity(10);
    for _ in 0..10 {
        let start = Instant::now();
        for _ in 0..75 {
            std::hint::black_box(backend.infer(std::hint::black_box(&input))?);
        }
        batches.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    batches.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({"backend": args[1], "threads": threads.get(),
        "first_load_ms": first_load_ms, "warm_load_ms": warm_load_ms,
        "synthetic_minute_median_ms": (batches[4] + batches[5]) / 2.0,
        "max_absolute_error": max_error, "model_bytes": model.metadata()?.len(),
        "binary_bytes": env::current_exe()?.metadata()?.len()})
    );
    io::stdout().flush()?;
    if env::var("DRDJ_ML_PROBE_HOLD").as_deref() == Ok("1") {
        let mut ack = String::new();
        io::stdin().read_line(&mut ack)?;
        if ack.trim() != "ACK" {
            return Err("memory probe acknowledgement missing".into());
        }
    }
    Ok(())
}
