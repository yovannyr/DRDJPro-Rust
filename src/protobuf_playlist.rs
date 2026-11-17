//! Protobuf conversion of the existing Playlist model, including entry identity.
use crate::playlist_manager::{Playlist, PlaylistEntry, PlaylistType};
use drdj_proto::{NativePlaylistMessage, PlaylistEntryMessage, PlaylistKind, PlaylistMessage};

fn timestamp(seconds: u64, field: &str) -> Result<drdj_proto::Timestamp, String> {
    let seconds = i64::try_from(seconds).map_err(|_| format!("{field}: timestamp overflow"))?;
    if seconds > 253_402_300_799 {
        return Err(format!("{field}: exceeds protobuf UTC range"));
    }
    Ok(drdj_proto::Timestamp { seconds, nanos: 0 })
}
fn seconds(value: Option<drdj_proto::Timestamp>, field: &str) -> Result<u64, String> {
    let value = value.ok_or_else(|| format!("{field}: required"))?;
    if value.nanos != 0 || !(0..=253_402_300_799).contains(&value.seconds) {
        return Err(format!(
            "{field}: native model requires whole nonnegative UTC seconds"
        ));
    }
    Ok(value.seconds as u64)
}

impl TryFrom<&Playlist> for PlaylistMessage {
    type Error = String;
    fn try_from(value: &Playlist) -> Result<Self, String> {
        let kind = match value.playlist_type {
            PlaylistType::User => PlaylistKind::User,
            PlaylistType::Smart => PlaylistKind::Smart,
            PlaylistType::RecentlyAdded => PlaylistKind::RecentlyAdded,
            PlaylistType::MostPlayed => PlaylistKind::MostPlayed,
            PlaylistType::Queue => PlaylistKind::Queue,
        };
        Ok(Self {
            name: value.name.clone(),
            track_order: value
                .entries
                .iter()
                .map(|e| e.track_id.to_string())
                .collect(),
            created_at: Some(timestamp(value.created_at, "created_at")?),
            modified_at: Some(timestamp(value.modified_at, "modified_at")?),
            native: Some(NativePlaylistMessage {
                id: value.id,
                kind: kind as i32,
                is_playing: value.is_playing,
                current_index: value.current_index as u64,
                description: value.description.clone(),
                cover_art_path: value.cover_art_path.clone(),
                entries: value
                    .entries
                    .iter()
                    .map(|e| {
                        Ok(PlaylistEntryMessage {
                            entry_id: e.entry_id,
                            track_id: e.track_id,
                            position: e.position as u64,
                            added_at: timestamp(e.added_at, "entries.added_at")?,
                        })
                    })
                    .collect::<Result<_, String>>()?,
            }),
        })
    }
}

impl TryFrom<PlaylistMessage> for Playlist {
    type Error = String;
    fn try_from(value: PlaylistMessage) -> Result<Self, String> {
        let native = value
            .native
            .ok_or("native: native playlist fields required")?;
        if value.track_order
            != native
                .entries
                .iter()
                .map(|e| e.track_id.to_string())
                .collect::<Vec<_>>()
        {
            return Err("track_order: does not match native entries".into());
        }
        let playlist_type = match PlaylistKind::try_from(native.kind)
            .map_err(|_| "native.kind: unknown enum value")?
        {
            PlaylistKind::User => PlaylistType::User,
            PlaylistKind::Smart => PlaylistType::Smart,
            PlaylistKind::RecentlyAdded => PlaylistType::RecentlyAdded,
            PlaylistKind::MostPlayed => PlaylistType::MostPlayed,
            PlaylistKind::Queue => PlaylistType::Queue,
            PlaylistKind::Unknown => return Err("native.kind: unknown playlist kind".into()),
        };
        Ok(Self {
            id: native.id,
            name: value.name,
            playlist_type,
            is_playing: native.is_playing,
            current_index: usize::try_from(native.current_index)
                .map_err(|_| "native.current_index: usize overflow")?,
            created_at: seconds(value.created_at, "created_at")?,
            modified_at: seconds(value.modified_at, "modified_at")?,
            description: native.description,
            cover_art_path: native.cover_art_path,
            entries: native
                .entries
                .into_iter()
                .map(|e| {
                    Ok(PlaylistEntry {
                        entry_id: e.entry_id,
                        track_id: e.track_id,
                        position: usize::try_from(e.position)
                            .map_err(|_| "entries.position: usize overflow")?,
                        added_at: seconds(Some(e.added_at), "entries.added_at")?,
                    })
                })
                .collect::<Result<_, String>>()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn playlist(kind: PlaylistType) -> Playlist {
        let mut p = Playlist::new(17, "Set 🎵".into(), kind);
        p.entries = vec![
            PlaylistEntry {
                entry_id: 99,
                track_id: 42,
                position: 1,
                added_at: 0,
            },
            PlaylistEntry {
                entry_id: 100,
                track_id: 42,
                position: 0,
                added_at: 100,
            },
        ];
        p.is_playing = true;
        p.current_index = 1;
        p.created_at = 0;
        p.modified_at = 123;
        p.description = Some(String::new());
        p.cover_art_path = Some("art.png".into());
        p
    }
    #[test]
    fn every_playlist_kind_preserves_all_fields_and_duplicate_track_references() {
        for kind in [
            PlaylistType::User,
            PlaylistType::Smart,
            PlaylistType::RecentlyAdded,
            PlaylistType::MostPlayed,
            PlaylistType::Queue,
        ] {
            let original = playlist(kind);
            let message = PlaylistMessage::try_from(&original).unwrap();
            assert_eq!(
                drdj_proto::from_json_checked::<PlaylistMessage>(
                    &drdj_proto::to_json_checked(&message).unwrap()
                )
                .unwrap(),
                message
            );
            let bytes = drdj_proto::encode_checked(&message).unwrap();
            let message: PlaylistMessage = drdj_proto::decode_checked(&bytes).unwrap();
            let restored = Playlist::try_from(message).unwrap();
            assert_eq!(
                PlaylistMessage::try_from(&restored).unwrap(),
                PlaylistMessage::try_from(&original).unwrap()
            );
            assert_eq!(restored.playlist_type, kind);
        }
    }
    #[test]
    fn rejects_unknown_enums_and_lossy_timestamp_conversions() {
        let mut p = PlaylistMessage::try_from(&playlist(PlaylistType::User)).unwrap();
        p.native.as_mut().unwrap().kind = 99;
        assert!(Playlist::try_from(p).is_err());
        let mut p = PlaylistMessage::try_from(&playlist(PlaylistType::User)).unwrap();
        p.created_at.as_mut().unwrap().nanos = 1;
        assert!(Playlist::try_from(p).is_err());
        let mut p = playlist(PlaylistType::User);
        p.created_at = u64::MAX;
        assert!(PlaylistMessage::try_from(&p).is_err());
    }
}
