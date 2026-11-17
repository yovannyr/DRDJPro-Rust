use drdj_proto::*;

pub fn track() -> TrackMessage {
    TrackMessage {
        title: "Präzision 🎵".into(),
        artist: "DJ".into(),
        album: "Live".into(),
        bpm: 128.1234567890123,
        key: "8A".into(),
        duration_ms: u64::MAX,
        file_path: "E:\\DJ_Music\\track.flac".into(),
        cover_art_bytes: vec![0, 1, 254, 255],
        analysis_hash: "sha256".into(),
        tempo: f32::from_bits(0x3f812345),
        beat_grid: Some(BeatGridMessage {
            bpm: 128.1234567890123,
            anchor_ns: 123_456_789,
            beats_per_bar: 4,
            meter_denominator: 4,
            warp_markers: vec![WarpMarkerMessage {
                beat: 5,
                position_ns: 987_654_321,
            }],
            tempo_changes: vec![TempoChangeMessage {
                beat: 6,
                bpm: 127.987654321,
            }],
            meter_changes: vec![MeterChangeMessage {
                bar: 4,
                beats: 3,
                denominator: 8,
            }],
            points: (0..500)
                .map(|i| BeatPointMessage {
                    position_ns: i * 500_000_000,
                    confidence: 0.9234567,
                })
                .collect(),
        }),
        key_change: None,
        cues: vec![CueMessage {
            position_ms: 42,
            label: "Intro".into(),
            color: 0x102030ff,
            point_index: 3,
            ..Default::default()
        }],
        loops: vec![LoopMessage {
            start_bar: 1,
            end_bar: 3,
            start_beat: 4,
            end_beat: 12,
            quantization: "Bar".into(),
            ..Default::default()
        }],
    }
}
