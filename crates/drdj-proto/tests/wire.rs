use drdj_proto::*;
use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage};
use serde::{de::DeserializeOwned, Serialize};

mod common;
use common::track;

fn roundtrip<T>(message: T)
where
    T: Message + Default + PartialEq + Serialize + DeserializeOwned,
{
    assert_eq!(decode::<T>(&encode(&message).unwrap()).unwrap(), message);
    assert_eq!(
        from_json::<T>(&to_json(&message).unwrap()).unwrap(),
        message
    );
}

#[test]
fn generated_messages_have_binary_and_json_roundtrips() {
    let t = track();
    roundtrip(t.clone());
    roundtrip(t.beat_grid.unwrap());
    roundtrip(t.cues[0].clone());
    roundtrip(t.loops[0].clone());
    roundtrip(PlaylistMessage {
        name: "Set".into(),
        track_order: vec!["b".into(), "a".into(), "b".into()],
        created_at: Some(pbjson_types::Timestamp {
            seconds: 0,
            nanos: 123_456_789,
        }),
        modified_at: Some(pbjson_types::Timestamp {
            seconds: 1_700_000_000,
            nanos: 0,
        }),
        native: None,
    });
    roundtrip(SyncStateMessage {
        revision: u64::MAX,
        last_sync: Some(pbjson_types::Timestamp {
            seconds: -1,
            nanos: 0,
        }),
        pending_changes: vec![0, 255, 128],
        server_version: 1,
    });
}

#[test]
fn binary_preserves_float_bits_and_nan_payloads() {
    for bits in [
        0,
        1,
        0x8000000000000000,
        0x7ff0000000000000,
        0x7ff8123456789abc,
    ] {
        let t = TrackMessage {
            bpm: f64::from_bits(bits),
            ..track()
        };
        let decoded: TrackMessage = decode(&encode(&t).unwrap()).unwrap();
        assert_eq!(decoded.bpm.to_bits(), bits);
        assert_eq!(decoded.tempo.to_bits(), t.tempo.to_bits());
    }
}

#[test]
fn protobuf_json_uses_base64_and_utc_timestamps() {
    let json: serde_json::Value = serde_json::from_str(&to_json(&track()).unwrap()).unwrap();
    assert_eq!(json["coverArtBytes"], "AAH+/w==");
    assert_eq!(json["durationMs"], u64::MAX.to_string());
    let playlist = PlaylistMessage {
        created_at: Some(pbjson_types::Timestamp {
            seconds: 0,
            nanos: 0,
        }),
        ..Default::default()
    };
    let json: serde_json::Value = serde_json::from_str(&to_json(&playlist).unwrap()).unwrap();
    assert_eq!(json["createdAt"], "1970-01-01T00:00:00+00:00");
}

#[test]
fn rejects_truncated_and_invalid_wire_values() {
    for bytes in [
        &[0x80][..],
        &[0x0a, 0x05, b'a'][..],
        &[0x0f][..],
        &[0x00][..],
    ] {
        assert!(decode::<TrackMessage>(bytes).is_err(), "accepted {bytes:?}");
    }
    assert!(from_json::<TrackMessage>("{broken").is_err());
}

#[test]
fn bounded_binary_and_json_inputs() {
    let oversized = vec![0; MAX_MESSAGE_BYTES + 1];
    assert!(matches!(
        decode::<TrackMessage>(&oversized),
        Err(ProtoError::TooLarge { .. })
    ));
    assert!(matches!(
        ForwardMessage::decode("drdj.v1.TrackMessage", &oversized),
        Err(ProtoError::TooLarge { .. })
    ));
    assert!(matches!(
        from_json::<TrackMessage>(&" ".repeat(MAX_MESSAGE_BYTES + 1)),
        Err(ProtoError::TooLarge { .. })
    ));
    let t = TrackMessage {
        cover_art_bytes: oversized,
        ..Default::default()
    };
    assert!(matches!(encode(&t), Err(ProtoError::TooLarge { .. })));
}

#[test]
fn relay_retains_unknown_top_level_and_nested_fields() {
    let mut bytes = encode(&track()).unwrap();
    // Future field 100 (varint), and another occurrence of nested grid with
    // its own future field 100. Protobuf merges repeated embedded messages.
    bytes.extend_from_slice(&[0xa0, 0x06, 42, 0x5a, 3, 0xa0, 0x06, 17]);
    let relay = ForwardMessage::decode("drdj.v1.TrackMessage", &bytes).unwrap();
    let forwarded = relay.encode().unwrap();
    let pool = DescriptorPool::decode(FILE_DESCRIPTOR_SET).unwrap();
    let dynamic = DynamicMessage::decode(
        pool.get_message_by_name("drdj.v1.TrackMessage").unwrap(),
        forwarded.as_slice(),
    )
    .unwrap();
    assert_eq!(dynamic.unknown_fields().count(), 1);
    let grid = dynamic.get_field_by_name("beat_grid").unwrap();
    let prost_reflect::Value::Message(grid) = grid.as_ref() else {
        panic!("missing grid")
    };
    assert_eq!(grid.unknown_fields().count(), 1);
    let decoded: TrackMessage = decode(&forwarded).unwrap();
    assert_eq!(decoded, track());
    assert!(matches!(
        ForwardMessage::decode("not.a.Message", &bytes),
        Err(ProtoError::UnknownMessage(_))
    ));
}

#[test]
fn additive_schema_evolution_preserves_older_fields() {
    let mut new = track();
    new.key_change = Some(3.5);
    let mut descriptor = prost_types_descriptor();
    let package = descriptor
        .file
        .iter_mut()
        .find(|f| f.package.as_deref() == Some("drdj.v1"))
        .unwrap();
    let old_track = package
        .message_type
        .iter_mut()
        .find(|m| m.name.as_deref() == Some("TrackMessage"))
        .unwrap();
    old_track.field.retain(|f| f.number != Some(12));
    let old_pool = DescriptorPool::from_file_descriptor_set(descriptor).unwrap();
    let old = DynamicMessage::decode(
        old_pool
            .get_message_by_name("drdj.v1.TrackMessage")
            .unwrap(),
        encode(&new).unwrap().as_slice(),
    )
    .unwrap();
    assert_eq!(old.unknown_fields().count(), 1);
    let restored: TrackMessage = decode(&old.encode_to_vec()).unwrap();
    assert_eq!(restored, new);
    let old_bytes = encode(&track()).unwrap();
    let current: TrackMessage = decode(&old_bytes).unwrap();
    assert_eq!(current.key_change(), 0.0);
    assert_eq!(current, track());
}

fn prost_types_descriptor() -> prost_types::FileDescriptorSet {
    prost_types::FileDescriptorSet::decode(FILE_DESCRIPTOR_SET).unwrap()
}

#[test]
fn thousand_cycles_preserve_all_values() {
    let original = track();
    let mut current = original.clone();
    for _ in 0..1000 {
        current = decode(&encode(&current).unwrap()).unwrap();
    }
    assert_eq!(current, original);
}
