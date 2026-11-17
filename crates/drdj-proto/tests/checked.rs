use drdj_proto::*;
use prost::Message;
mod common;

#[test]
fn checked_codecs_roundtrip_and_reject_absent_required_fields() {
    let track = common::track();
    assert_eq!(
        decode_checked::<TrackMessage>(&encode_checked(&track).unwrap()).unwrap(),
        track
    );
    assert_eq!(
        from_json_checked::<TrackMessage>(&to_json_checked(&track).unwrap()).unwrap(),
        track
    );
    let error = decode_checked::<TrackMessage>(&[]).unwrap_err().to_string();
    assert!(error.contains("required field missing"));
    // Present message title with wrong wire type; and an absent nested scalar.
    assert!(decode_checked::<TrackMessage>(&[0x08, 1]).is_err());
    let mut complete = track.encode_to_vec();
    // Append a new cue with no required fields. It must not turn into defaults.
    complete.extend_from_slice(&[0x6a, 0]);
    let error = decode_checked::<TrackMessage>(&complete)
        .unwrap_err()
        .to_string();
    assert!(error.contains("cues[1].position_ms"), "{error}");
}

#[test]
fn rejects_nested_nonfinite_values_and_invalid_timestamps_before_serialization() {
    let mut track = common::track();
    track.beat_grid.as_mut().unwrap().points[12].confidence = f32::NAN;
    let error = encode_checked(&track).unwrap_err().to_string();
    assert!(error.contains("beat_grid.points[12].confidence"));
    let state = SyncStateMessage {
        last_sync: Some(Timestamp {
            seconds: 0,
            nanos: -1,
        }),
        ..Default::default()
    };
    assert!(encode_checked(&state)
        .unwrap_err()
        .to_string()
        .contains("last_sync"));
    let json = r#"{"bpm":"not-a-number","filePath":"private-track"}"#;
    let error = from_json::<TrackMessage>(json).unwrap_err().to_string();
    assert!(error.contains("bpm"));
    assert!(from_json::<TrackMessage>("{} {}").is_err());
}

#[test]
fn sync_batch_and_version_error_have_real_roundtrips() {
    let mut track = common::track();
    track.beat_grid = None; // Metadata updates, not 100 full analysis grids.
    let request = SyncRequest {
        schema_version: SCHEMA_VERSION,
        updates: vec![track; 100],
        state: Some(SyncStateMessage {
            revision: 42,
            ..Default::default()
        }),
        ..Default::default()
    };
    let encoded = encode_checked(&request).unwrap();
    assert!(encoded.len() < 200_000, "{} bytes", encoded.len());
    let decoded = decode_checked(&encoded).unwrap();
    let response = respond(decoded);
    assert_eq!(response.updates, request.updates);
    assert_eq!(response.state.unwrap().revision, 42);
    let mismatch = respond(SyncRequest {
        schema_version: SCHEMA_VERSION + 1,
        ..request
    });
    assert!(mismatch.updates.is_empty());
    assert!(mismatch.state.is_none());
    assert!(mismatch.version_error.is_some());
    assert_eq!(
        decode_checked::<SyncResponse>(&encode_checked(&mismatch).unwrap()).unwrap(),
        mismatch
    );
}

#[test]
fn pad_bank_bounds_size_and_thousand_cycles() {
    let original = PadBankMessage {
        pads: (0..16)
            .map(|index| PadStateMessage {
                index,
                active: index % 2 == 0,
                color: 0x123456ff,
            })
            .collect(),
    };
    let bytes = encode_checked(&original).unwrap();
    assert!(bytes.len() < 1024);
    let mut current = original.clone();
    for _ in 0..1000 {
        current = decode_checked(&encode_checked(&current).unwrap()).unwrap();
    }
    assert_eq!(current, original);
    let mut invalid = original.clone();
    invalid.pads.push(invalid.pads[0]);
    assert!(encode_checked(&invalid).is_err());
    let mut invalid = original;
    invalid.pads[0].index = 16;
    assert!(encode_checked(&invalid).is_err());
}

#[test]
fn wire_merging_remains_valid_when_required_nested_fields_are_split() {
    // Required fields of a singular embedded message may be spread over
    // occurrences. Structural validation must inspect the merged message.
    let original = SyncRequest {
        schema_version: 1,
        updates: Vec::new(),
        state: Some(SyncStateMessage::default()),
        ..Default::default()
    };
    let mut bytes = encode_checked(&original).unwrap();
    bytes.extend_from_slice(&[0x1a, 2, 0x08, 7]); // state.revision = 7
    assert_eq!(
        decode_checked::<SyncRequest>(&bytes)
            .unwrap()
            .state
            .unwrap()
            .revision,
        7
    );
}

#[test]
fn representative_six_message_types_reduce_payloads_by_forty_percent() {
    fn compare<T: Contract + serde::Serialize>(name: &str, value: &T) {
        let binary = encode_checked(value).unwrap().len();
        let json = to_json_checked(value).unwrap().len();
        println!(
            "{name}: protobuf={binary} JSON={json} reduction={:.2}%",
            100.0 * (1.0 - binary as f64 / json as f64)
        );
        assert!(binary * 100 <= json * 60, "{name}: {binary} vs {json}");
    }
    let t = common::track();
    compare("track-500-points", &t);
    compare("beat-grid-500-points", t.beat_grid.as_ref().unwrap());
    compare("cue", &t.cues[0]);
    compare("loop", &t.loops[0]);
    compare(
        "playlist",
        &PlaylistMessage {
            name: "Set".into(),
            track_order: (0..10).map(|i| i.to_string()).collect(),
            created_at: Some(Timestamp {
                seconds: 1_700_000_000,
                nanos: 0,
            }),
            modified_at: Some(Timestamp {
                seconds: 1_700_000_100,
                nanos: 0,
            }),
            native: None,
        },
    );
    compare(
        "sync-state",
        &SyncStateMessage {
            revision: 42,
            last_sync: Some(Timestamp {
                seconds: 1_700_000_000,
                nanos: 0,
            }),
            pending_changes: vec![1, 2, 3, 4],
            server_version: 2,
        },
    );
}

#[test]
fn sync_relay_preserves_future_fields_inside_track_updates() {
    use prost_reflect::{DescriptorPool, DynamicMessage, Value};
    let mut track = encode_checked(&common::track()).unwrap();
    track.extend_from_slice(&[0xa0, 0x06, 42]);
    let mut request = vec![0x08, 1];
    prost::encoding::bytes::encode(2, &track, &mut request);
    request.extend_from_slice(&[0xa0, 0x06, 17]);
    let forwarded = ForwardMessage::decode("drdj.v1.SyncRequest", &request)
        .unwrap()
        .encode()
        .unwrap();
    let pool = DescriptorPool::decode(FILE_DESCRIPTOR_SET).unwrap();
    let dynamic = DynamicMessage::decode(
        pool.get_message_by_name("drdj.v1.SyncRequest").unwrap(),
        forwarded.as_slice(),
    )
    .unwrap();
    assert_eq!(dynamic.unknown_fields().count(), 1);
    let field = dynamic.get_field_by_name("updates").unwrap();
    let Value::List(updates) = field.as_ref() else {
        panic!("expected updates")
    };
    let Value::Message(track) = &updates[0] else {
        panic!("expected track")
    };
    assert_eq!(track.unknown_fields().count(), 1);
    assert_eq!(
        decode_checked::<SyncRequest>(&forwarded).unwrap().updates[0],
        common::track()
    );
}

#[test]
fn presence_scan_matches_reflection_on_merging_groups_and_malformed_wire_corpus() {
    use prost_reflect::{Cardinality, DescriptorPool, DynamicMessage, ReflectMessage, Value};
    fn complete(value: &DynamicMessage) -> bool {
        value
            .descriptor()
            .fields()
            .all(|field| field.cardinality() != Cardinality::Required || value.has_field(&field))
            && value.fields().all(|(_, value)| match value {
                Value::Message(child) => complete(child),
                Value::List(children) => children.iter().all(|value| match value {
                    Value::Message(child) => complete(child),
                    _ => true,
                }),
                _ => true,
            })
    }
    let pool = DescriptorPool::decode(FILE_DESCRIPTOR_SET).unwrap();
    let descriptor = pool.get_message_by_name("drdj.v1.SyncRequest").unwrap();
    let compare = |bytes: &[u8]| {
        let reference = DynamicMessage::decode(descriptor.clone(), bytes)
            .ok()
            .filter(complete)
            .and_then(|_| SyncRequest::decode(bytes).ok())
            .is_some_and(|value| value.validate().is_ok());
        assert_eq!(
            decode_checked::<SyncRequest>(bytes).is_ok(),
            reference,
            "wire {bytes:?}"
        );
    };
    // Two incomplete singular state fragments merge into one complete state.
    let merged = [8, 1, 0x1a, 2, 8, 42, 0x1a, 4, 0x1a, 0, 0x20, 1];
    assert!(decode_checked::<SyncStateMessage>(&[8, 42]).is_err());
    assert!(decode_checked::<SyncStateMessage>(&[0x1a, 0, 0x20, 1]).is_err());
    assert!(decode_checked::<SyncRequest>(&merged).is_ok());
    compare(&merged);
    // Repeated siblings are separate messages, never fragments of one value.
    let track = TrackMessage {
        title: "x".into(),
        file_path: "track.wav".into(),
        ..Default::default()
    };
    let encoded = encode_checked(&track).unwrap();
    assert_eq!(&encoded[..3], &[10, 1, b'x']);
    let mut repeated = vec![8, 1];
    prost::encoding::bytes::encode(2, &encoded[..3].to_vec(), &mut repeated);
    prost::encoding::bytes::encode(2, &encoded[3..].to_vec(), &mut repeated);
    assert!(decode_checked::<SyncRequest>(&repeated).is_err());
    compare(&repeated);
    for suffix in [
        vec![0xa3, 6, 8, 1, 0xa4, 6],
        vec![0xa3, 6, 8, 1, 0xac, 6],
        vec![0xa4, 6],
        vec![0x1a, 0x80],
        vec![0x80],
        vec![0x1a, 255, 255, 255, 255, 255, 255, 255, 255, 255, 2],
    ] {
        let mut bytes = vec![8, 1];
        bytes.extend(suffix);
        compare(&bytes);
    }
    let mut deep = vec![8, 1];
    for _ in 0..105 {
        deep.extend_from_slice(&[0xa3, 6]);
    }
    for _ in 0..105 {
        deep.extend_from_slice(&[0xa4, 6]);
    }
    assert!(decode_checked::<SyncRequest>(&deep).is_err());
    compare(&deep);
    for end in 0..merged.len() {
        compare(&merged[..end]);
    }
    for byte in 0..=255 {
        compare(&[8, 1, byte]);
        compare(&[8, 1, byte, 0]);
    }
}

#[test]
fn exact_four_mebibyte_limit_is_supported_but_larger_messages_are_rejected() {
    let mut state = SyncStateMessage {
        pending_changes: vec![42; MAX_MESSAGE_BYTES],
        ..Default::default()
    };
    let overhead = state.encoded_len() - state.pending_changes.len();
    state.pending_changes.truncate(MAX_MESSAGE_BYTES - overhead);
    let bytes = encode_checked(&state).unwrap();
    assert_eq!(bytes.len(), MAX_MESSAGE_BYTES);
    assert_eq!(decode_checked::<SyncStateMessage>(&bytes).unwrap(), state);
    state.pending_changes.push(42);
    assert!(matches!(
        encode_checked(&state),
        Err(ProtoError::TooLarge { .. })
    ));
}

#[test]
fn four_cues_and_loop_quantization_survive_a_thousand_cycles() {
    let mut original = common::track();
    original.cues = (0..4)
        .map(|point_index| CueMessage {
            position_ms: u64::from(point_index) * 12345,
            label: format!("Cue {point_index}"),
            color: 0x123456ff + point_index,
            point_index,
            ..Default::default()
        })
        .collect();
    original.loops[0].quantization = "1/4 beat".into();
    let mut current = original.clone();
    for _ in 0..1000 {
        current = decode_checked(&encode_checked(&current).unwrap()).unwrap();
    }
    assert_eq!(current, original);
}
