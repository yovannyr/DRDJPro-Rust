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
use drdj_dvs::Record;

pub fn signal(record: Record, rate: u32, start: usize, seconds: f64, speed: f64) -> Vec<i16> {
    let (mut state, taps) = match record {
        Record::SeratoSecondEditionA => (0x59017u32, 0x361e4u32),
        Record::SeratoSecondEditionB => (0x8f3c6, 0x4f0d8),
        Record::SeratoCd => (0xd8b40, 0x34d54),
    };
    let mut cycle = 0;
    let mut pcm = Vec::new();
    for frame in 0..(seconds * f64::from(rate)) as usize {
        let time = start as f64 + frame as f64 * speed / f64::from(rate);
        let next_cycle = (time * 1000.0).floor() as usize;
        while cycle < next_cycle {
            let bit = (state & (taps | 1)).count_ones() & 1;
            state = (state >> 1) | (bit << 19);
            cycle += 1;
        }
        let phase = time * 1000.0 * std::f64::consts::TAU;
        let modulation = if state & 1 == 0 {
            1.0 - (1.0 - phase.cos()) * 0.25
        } else {
            1.0
        };
        pcm.push((-phase.cos() * modulation * 16000.0) as i16);
        pcm.push((phase.sin() * modulation * 16000.0) as i16);
    }
    pcm
}
