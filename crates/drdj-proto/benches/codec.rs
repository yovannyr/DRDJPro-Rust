//! Acceptance measurement: median per-operation time from 31 batches of 100.
use drdj_proto::*;
use std::{hint::black_box, time::Instant};
#[path = "../tests/common/mod.rs"]
mod common;

fn measure<T>(name: &str, limit_us: f64, mut operation: impl FnMut() -> T) {
    for _ in 0..100 {
        black_box(operation());
    }
    let mut samples = Vec::new();
    for _ in 0..31 {
        let start = Instant::now();
        for _ in 0..100 {
            black_box(operation());
        }
        samples.push(start.elapsed().as_secs_f64() * 1_000_000.0 / 100.0);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{name}: median={:.3} us, p95 batch={:.3} us, limit={limit_us} us",
        samples[15], samples[29]
    );
    assert!(samples[15] < limit_us, "{name} exceeds acceptance limit");
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("Timing acceptance runs separately: cargo bench -p drdj-proto --bench codec");
        return;
    }
    let t = common::track();
    let bytes = encode_checked(&t).unwrap();
    measure("validated track encode (500 points)", 50.0, || {
        encode_checked(black_box(&t)).unwrap()
    });
    measure("validated track decode (500 points)", 100.0, || {
        decode_checked::<TrackMessage>(black_box(&bytes)).unwrap()
    });
    let pads = PadBankMessage {
        pads: (0..16)
            .map(|index| PadStateMessage {
                index,
                active: true,
                color: 0x123456ff,
            })
            .collect(),
    };
    measure("validated 16-pad encode", 10.0, || {
        encode_checked(black_box(&pads)).unwrap()
    });
}
