use drdj_itunes::*;
use std::{io::Cursor, time::Instant};
fn doc(tracks: &str, playlists: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>Tracks</key><dict>{tracks}</dict><key>Playlists</key><array>{playlists}</array></dict></plist>"#
    )
}
fn track(id: u64, location: &str, extra: &str) -> String {
    let location = quick_xml::escape::escape(location);
    format!("<key>{id}</key><dict><key>Track ID</key><integer>{id}</integer><key>Location</key><string>{location}</string><key>Name</key><string>A&amp;B é</string><key>Artist</key><string>Artist</string>{extra}</dict>")
}
fn parse_str(xml: &str) -> Result<Import, String> {
    parse(Cursor::new(xml.as_bytes()), &|| false)
}
#[test]
fn all_metadata_entities_empty_values_and_order_are_preserved() {
    let xml=doc(&track(7,"file:///C:/Music/test.mp3","<key>Album</key><string>Album</string><key>Album Artist</key><string>Group</string><key>Genre</key><string>House</string><key>BPM</key><real>128.5</real><key>Rating</key><integer>80</integer><key>Play Count</key><integer>12</integer><key>Total Time</key><integer>123456</integer><key>Date Added</key><date>2020-01-02T03:04:05Z</date><key>Persistent ID</key><string>ABC123</string>"),"<dict><key>Playlist ID</key><integer>3</integer><key>Name</key><string>Mix</string><key>Playlist Items</key><array><dict><key>Track ID</key><integer>7</integer></dict><dict><key>Track ID</key><integer>7</integer></dict></array></dict>");
    let p = parse_str(&xml).unwrap();
    let t = &p.tracks[0];
    assert_eq!(t.id, 7);
    assert_eq!(t.title, "A&B é");
    assert_eq!(t.artist, "Artist");
    assert_eq!(t.album, "Album");
    assert_eq!(t.album_artist, "Group");
    assert_eq!(t.genre, "House");
    assert_eq!(t.bpm, Some(128.5));
    assert_eq!(t.rating, Some(80));
    assert_eq!(t.play_count, Some(12));
    assert_eq!(t.duration_ms, Some(123456));
    assert_eq!(t.date_added.as_deref(), Some("2020-01-02T03:04:05Z"));
    assert_eq!(t.persistent_id, "ABC123");
    assert_eq!(p.playlists[0].tracks, [7, 7]);
    assert_eq!(p.skipped, 0);
    assert_eq!(parse_str(&doc("", "")).unwrap().tracks.len(), 0);
}
#[test]
fn bad_semantic_entries_and_drm_are_skipped_without_losing_healthy_tracks() {
    let tracks = [
        track(1, "file:///a.mp3", "<key>Protected</key><true></true>"),
        track(
            2,
            "file:///b.mp3",
            "<key>Kind</key><string>Apple Music AAC audio file</string>",
        ),
        track(3, "file:///c.m4p", ""),
        track(
            4,
            "file:///d.mp3",
            "<key>Rating</key><integer>oops</integer>",
        ),
        track(5, "file:///ok.mp3", ""),
    ]
    .concat();
    let p = parse_str(&doc(&tracks, "<dict><key>Master</key><true/></dict>")).unwrap();
    assert_eq!(p.tracks.len(), 1);
    assert_eq!(p.tracks[0].id, 5);
    assert_eq!(p.skipped, 4);
    assert_eq!(p.warnings.len(), 4);
}
#[test]
fn malformed_or_hostile_xml_aborts_and_cancellation_is_observed() {
    for xml in [
        "bplist00foo",
        "<plist><dict><key>Tracks</key><dict>",
        "<plist><dict><key>Tracks</key><string>wrong</string></dict></plist>",
        "<!DOCTYPE plist [<!ENTITY x 'x'>]><plist><dict/></plist>",
        "<plist><dict><key>Tracks</key><dict/></dict></plist><extra/>",
    ] {
        assert!(parse_str(xml).is_err(), "{xml}");
    }
    let xml = doc(&track(1, "file:///a.mp3", ""), "");
    assert!(parse(Cursor::new(xml), &|| true).is_err());
    let duplicate = doc(
        &[track(1, "file:///a.mp3", ""), track(1, "file:///b.mp3", "")].concat(),
        "",
    );
    assert!(parse_str(&duplicate).is_err());
}
#[test]
fn file_inspection_deduplicates_preserves_playlist_occurrences_and_marks_missing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("é & one.mp3");
    std::fs::write(&file, b"fixture").unwrap();
    let uri = url::Url::from_file_path(&file).unwrap().to_string();
    let missing = url::Url::from_file_path(dir.path().join("missing.flac"))
        .unwrap()
        .to_string();
    let tracks = [
        track(1, &uri, ""),
        track(2, &uri, ""),
        track(3, &missing, ""),
    ]
    .concat();
    let playlist="<dict><key>Playlist ID</key><integer>1</integer><key>Name</key><string>P</string><key>Playlist Items</key><array><dict><key>Track ID</key><integer>3</integer></dict><dict><key>Track ID</key><integer>2</integer></dict><dict><key>Track ID</key><integer>1</integer></dict><dict><key>Track ID</key><integer>99</integer></dict></array></dict>";
    let p = inspect(parse_str(&doc(&tracks, playlist)).unwrap(), None, &|| false).unwrap();
    assert_eq!(p.tracks.len(), 2);
    assert_eq!(p.duplicates, 1);
    assert_eq!(p.missing, 1);
    assert_eq!(p.unresolved_links, 1);
    assert_eq!(p.playlists[0].tracks, [3, 1, 1]);
    assert_eq!(p.tracks[0].file.size, Some(7));
    assert!(p.tracks[0].file.modified_ms.is_some());
    assert_eq!(
        std::fs::canonicalize(p.tracks[0].file.path().unwrap()).unwrap(),
        std::fs::canonicalize(file).unwrap()
    );
}
#[test]
fn media_root_remaps_full_relative_structure_and_rejects_unsafe_or_missing_base() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Artist")).unwrap();
    std::fs::write(dir.path().join("Artist/one.mp3"), b"x").unwrap();
    let mut p = parse_str(&doc(&track(1, "file:///old/Artist/one.mp3", ""), "")).unwrap();
    p.music_folder = Some("file:///old/".into());
    let prepared = inspect(p.clone(), Some(dir.path()), &|| false).unwrap();
    assert!(prepared.tracks[0].file.available);
    p.music_folder = None;
    assert!(inspect(p.clone(), Some(dir.path()), &|| false).is_err());
    p.music_folder = Some("file:///old/".into());
    p.tracks[0].location = "file:///old/Artist/%2F..%2Fsecret.mp3".into();
    let p = inspect(p, Some(dir.path()), &|| false).unwrap();
    assert!(p.tracks.is_empty());
    assert_eq!(p.skipped, 1);
}
#[test]
fn encoded_drm_and_remote_locations_never_become_playable_files() {
    let p = parse_str(&doc(
        &[
            track(1, "file:///C:/secret%2Em4p", ""),
            track(2, "https://example.org/song.mp3", ""),
        ]
        .concat(),
        "",
    ))
    .unwrap();
    let p = inspect(p, None, &|| false).unwrap();
    assert!(p.tracks.is_empty());
    assert_eq!(p.skipped, 2);
}
#[test]
fn cancellation_after_last_file_and_during_large_playlist_does_not_return_success() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("last.mp3"), b"x").unwrap();
    let uri = url::Url::from_file_path(dir.path().join("last.mp3"))
        .unwrap()
        .to_string();
    let p = parse_str(&doc(&track(1, &uri, ""), "")).unwrap();
    let calls = std::cell::Cell::new(0);
    assert!(inspect(p, None, &|| {
        calls.set(calls.get() + 1);
        calls.get() >= 5
    })
    .is_err());
    assert_eq!(calls.get(), 5);
    let p = Import {
        playlists: vec![Playlist {
            id: "1".into(),
            name: "large".into(),
            tracks: vec![1; 3000],
        }],
        ..Default::default()
    };
    calls.set(0);
    assert!(inspect(p, None, &|| {
        calls.set(calls.get() + 1);
        calls.get() >= 3
    })
    .is_err());
    assert_eq!(calls.get(), 3);
    let uri = url::Url::from_file_path(dir.path().join("missing/deep/file.mp3"))
        .unwrap()
        .to_string();
    let p = parse_str(&doc(&track(1, &uri, ""), "")).unwrap();
    calls.set(0);
    assert!(inspect(p, None, &|| {
        calls.set(calls.get() + 1);
        calls.get() >= 5
    })
    .is_err());
    assert_eq!(calls.get(), 6);
}
#[cfg(windows)]
#[test]
fn file_uri_uses_stable_native_identity_before_and_after_file_appears() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("later.mp3");
    let uri = url::Url::from_file_path(&path).unwrap().to_string();
    let prepared = inspect(
        parse_str(&doc(&track(1, &uri, ""), "")).unwrap(),
        None,
        &|| false,
    )
    .unwrap();
    let info = &prepared.tracks[0].file;
    let before = info.path().unwrap();
    std::fs::write(&path, b"x").unwrap();
    assert_eq!(before, std::fs::canonicalize(path).unwrap());
}
#[test]
fn hundred_thousand_plus_tracks_are_parsed_without_a_full_document_tree() {
    let mut tracks = String::new();
    for id in 1..=100_001 {
        tracks.push_str(&track(id, &format!("file:///C:/Music/{id}.mp3"), ""));
        if id == 50_000 {
            let xml = doc(&tracks, "");
            let now = Instant::now();
            let p = parse_str(&xml).unwrap();
            let elapsed = now.elapsed();
            assert_eq!(p.tracks.len(), 50_000);
            eprintln!("iTunes50000tracks parsed in{elapsed:?}");
            assert!(elapsed.as_secs() < 30);
        }
    }
    let xml = doc(&tracks, "");
    drop(tracks);
    let now = Instant::now();
    let p = parse_str(&xml).unwrap();
    let elapsed = now.elapsed();
    eprintln!(
        "iTunes100001tracks XML{}bytes parsed in{elapsed:?}",
        xml.len()
    );
    assert_eq!(p.tracks.len(), 100_001);
    assert!(
        elapsed.as_secs() < 60,
        "isolated parser budget; no filesystem/UI claim"
    );
}
