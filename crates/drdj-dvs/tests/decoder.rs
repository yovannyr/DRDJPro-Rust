//! GPL-3.0-only. Synthetic AM generator adapted from Mark Hills' xwax
//! mktimecode.c at be863572137929f70aa3ff0cef3b6b76f4973005.
//! This verifies digital contracts, not physical Serato vinyl compatibility.
/*
 * Copyright (C) 2026 Mark Hills <mark@xwax.org>
 * Adapted from xwax mktimecode.c by DRDJPro, 2026-11-12.
 *
 * This program is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License, version 3 as
 * published by the Free Software Foundation.
 *
 * This program is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */
use drdj_dvs::*;
mod support;
use support::signal;

fn feed(decoder: &mut Decoder, pcm: &[i16]) -> Option<Observation> {
    let mut last = None;
    for block in pcm.chunks(512) {
        last = decoder.submit(block).unwrap();
    }
    last
}
#[test]
fn named_editions_decode_absolute_words_at_multiple_sample_rates() {
    for record in [
        Record::SeratoSecondEditionA,
        Record::SeratoSecondEditionB,
        Record::SeratoCd,
    ] {
        for rate in [44100, 48000, 96000] {
            let mut decoder = Decoder::new(record, rate, false).unwrap();
            let observation = feed(&mut decoder, &signal(record, rate, 3, 2.0, 1.0))
                .expect("valid encoded words");
            // Twenty-bit windows identify their first bit: 19ms before the last
            // decoded bit. This is upstream's record-time origin, not latency.
            assert!(
                (observation.position_seconds - 4.981).abs() < 0.004,
                "{record:?} {rate}: {observation:?}"
            );
            assert!((observation.speed - 1.0).abs() < 0.003, "{observation:?}");
        }
    }
}
#[test]
fn five_percent_slowdown_is_measured_instead_of_simulated() {
    let record = Record::SeratoSecondEditionA;
    let mut decoder = Decoder::new(record, 48000, false).unwrap();
    let observation = feed(&mut decoder, &signal(record, 48000, 0, 2.0, 0.95)).unwrap();
    assert!((observation.speed - 0.95).abs() < 0.003, "{observation:?}");
    assert!(
        (observation.position_seconds - 1.881).abs() < 0.004,
        "{observation:?}"
    );
}
#[test]
fn reverse_and_absolute_needle_jump_follow_encoded_position() {
    let record = Record::SeratoSecondEditionA;
    let mut decoder = Decoder::new(record, 48000, false).unwrap();
    let forward = signal(record, 48000, 3, 2.0, 1.0);
    let reverse: Vec<_> = forward
        .as_chunks::<2>()
        .0
        .iter()
        .rev()
        .flatten()
        .copied()
        .collect();
    let observation = feed(&mut decoder, &reverse).unwrap();
    assert!((observation.speed + 1.0).abs() < 0.003, "{observation:?}");
    assert!(
        (observation.position_seconds - 3.0).abs() < 0.025,
        "{observation:?}"
    );
    let jump = feed(&mut decoder, &signal(record, 48000, 20, 2.0, 1.0)).unwrap();
    assert!((jump.position_seconds - 21.981).abs() < 0.004, "{jump:?}");
}
#[test]
fn silence_noise_plain_tone_and_reset_do_not_produce_valid_positions() {
    let record = Record::SeratoSecondEditionA;
    let mut decoder = Decoder::new(record, 48000, false).unwrap();
    assert!(feed(&mut decoder, &signal(record, 48000, 0, 1.0, 1.0)).is_some());
    assert!(feed(&mut decoder, &vec![0; 4800]).is_none());
    decoder.reset();
    assert!(decoder.submit(&[0; 512]).unwrap().is_none());
    let mut noise = 0x12345678u32;
    for _ in 0..200 {
        let mut block = [0; 512];
        for sample in &mut block {
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            *sample = noise as i16;
        }
        assert!(decoder.submit(&block).unwrap().is_none());
    }
    decoder.reset();
    let tone: Vec<_> = (0..48000)
        .flat_map(|frame| {
            let phase = f64::from(frame) * std::f64::consts::TAU / 48.0;
            [
                (-phase.cos() * 16000.0) as i16,
                (phase.sin() * 16000.0) as i16,
            ]
        })
        .collect();
    assert!(feed(&mut decoder, &tone).is_none());
}
#[test]
fn malformed_and_unbounded_input_is_rejected_before_native_decode() {
    for rate in [0, 7999, 192001, u32::MAX] {
        assert!(matches!(
            Decoder::new(Record::SeratoCd, rate, false),
            Err(Error::InvalidRate)
        ));
    }
    let mut decoder = Decoder::new(Record::SeratoCd, 48000, false).unwrap();
    for block in [vec![], vec![1], vec![1; 2 * MAX_BLOCK_FRAMES + 2]] {
        assert!(matches!(decoder.submit(&block), Err(Error::InvalidBlock)));
    }
    assert!(decoder
        .submit(&vec![0; 2 * MAX_BLOCK_FRAMES])
        .unwrap()
        .is_none());
}

#[test]
fn decoder_processing_measurement_uses_encoded_stereo_blocks() {
    let pcm = signal(Record::SeratoSecondEditionA, 48000, 3, 2.0, 1.0);
    let mut decoder = Decoder::new(Record::SeratoSecondEditionA, 48000, false).unwrap();
    let mut times = Vec::with_capacity(7500);
    let mut valid = 0;
    for _ in 0..20 {
        for block in pcm.chunks(512) {
            let start = std::time::Instant::now();
            let result = decoder.submit(block).unwrap();
            let elapsed = start.elapsed();
            valid += usize::from(result.is_some());
            times.push(elapsed.as_nanos());
        }
    }
    assert!(valid > 7000);
    times.sort_unstable();
    println!("xwax decoder only, 256 stereo frames / 48kHz, {} blocks: p50={}ns p99={}ns; no hardware/capture/UI included",
        times.len(), times[times.len() / 2], times[times.len() * 99 / 100]);
}

#[test]
fn phono_low_level_decoding_and_explicit_channel_order() {
    let record = Record::SeratoSecondEditionA;
    let pcm = signal(record, 48000, 3, 2.0, 1.0);
    let quiet: Vec<_> = pcm
        .iter()
        .map(|sample| (f64::from(*sample) * 0.01) as i16)
        .collect();
    let mut phono = Decoder::new(record, 48000, true).unwrap();
    assert!(feed(&mut phono, &quiet).is_some());
    let swapped: Vec<_> = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .flat_map(|[left, right]| [*right, *left])
        .collect();
    phono.reset();
    assert!(
        feed(&mut phono, &swapped).is_none(),
        "swapped channels must not fabricate forward position"
    );
    phono.reset();
    assert!(feed(&mut phono, &pcm).is_some());
}
#[test]
fn full_scale_dc_safe_area_and_reacquisition_after_loss() {
    let record = Record::SeratoSecondEditionA;
    let mut decoder = Decoder::new(record, 48000, false).unwrap();
    for level in [i16::MIN, i16::MAX, 0, -1, 1] {
        decoder.reset();
        assert!(decoder
            .submit(&vec![level; MAX_BLOCK_FRAMES * 2])
            .unwrap()
            .is_none());
    }
    assert!(feed(&mut decoder, &signal(record, 48000, 706, 0.5, 1.0)).is_some());
    assert!(
        feed(&mut decoder, &signal(record, 48000, 708, 0.5, 1.0)).is_none(),
        "beyond upstream safe area"
    );
    decoder.reset();
    assert!(feed(&mut decoder, &signal(record, 48000, 3, 1.0, 1.0)).is_some());
    assert!(feed(&mut decoder, &vec![0; 4800]).is_none());
    let pcm = signal(record, 48000, 20, 0.5, 1.0);
    let acquired = pcm
        .chunks(512)
        .position(|block| decoder.submit(block).unwrap().is_some());
    assert!(
        acquired.is_some(),
        "valid signal must reacquire within 500ms"
    );
}
