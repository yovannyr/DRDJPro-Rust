//! UI-thread bridge from inspected source records to the shared library.
use super::*;
impl AudioController {
    pub(crate) fn itunes_ready(&self) -> bool {
        self.storage_ready.get() && self.pending_import.borrow().is_none()
    }
    pub(crate) fn apply_itunes_record(
        &self,
        row: &drdj_itunes::PreparedTrack,
        source_changed: bool,
    ) -> Result<bool, String> {
        if !self.itunes_ready() {
            return Err("Wait for the current library operation".into());
        }
        let path = row.file.path()?;
        let overwrite = source_changed
            || self
                .library
                .borrow()
                .track_id_for_path(&path)
                .and_then(|id| {
                    self.library
                        .borrow()
                        .track_by_id(id)
                        .map(|t| !t.metadata.prefer_imported_tags)
                })
                .unwrap_or(true);
        let metadata = if overwrite {
            crate::track_metadata::TrackMetadata {
                prefer_imported_tags: true,
                title: row.track.title.clone(),
                artist: row.track.artist.clone(),
                album: row.track.album.clone(),
                duration_ms: row.track.duration_ms,
                scanned: true,
                ..Default::default()
            }
        } else {
            Default::default()
        };
        let changed = self.library.borrow_mut().import_source_track(
            path,
            row.file.available,
            metadata,
            overwrite,
        )?;
        if changed {
            self.library_changed();
        }
        Ok(changed)
    }
    pub(crate) fn publish_itunes_changes(&self) {
        self.publish_library();
        self.refresh();
    }
    pub(crate) fn load_itunes_track(&self, deck: usize, path: PathBuf) -> Result<(), String> {
        let id = match deck {
            0 => DeckId::A,
            1 => DeckId::B,
            _ => return Err("Invalid deck".into()),
        };
        if !self.storage_ready.get()
            || self.pending.borrow().is_some()
            || self.pending_import.borrow().is_some()
        {
            return Err("Wait for the current library/load operation".into());
        }
        let available = self
            .library
            .borrow()
            .track_id_for_path(&path)
            .and_then(|id| self.library.borrow().track_by_id(id).map(|t| t.available))
            .unwrap_or(false);
        if !available {
            return Err("Track is unavailable or not yet in the shared library".into());
        }
        if self.engine.borrow().snapshot(id).playing {
            return Err("Pause the target deck before replacing its track".into());
        }
        self.start_load(id, Arc::new(move || Some(path.clone())), 0.);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source(path: &Path) -> drdj_itunes::PreparedTrack {
        let location = url::Url::from_file_path(path).unwrap().to_string();
        let input = drdj_itunes::Import {
            tracks: vec![drdj_itunes::Track {
                id: 1,
                persistent_id: "P1".into(),
                location,
                title: "XML title".into(),
                artist: "XML artist".into(),
                album: "XML album".into(),
                album_artist: "Group".into(),
                genre: "House".into(),
                bpm: Some(128.),
                rating: Some(80),
                play_count: Some(12),
                duration_ms: Some(123456),
                date_added: Some("2020-01-02T03:04:05Z".into()),
            }],
            ..Default::default()
        };
        drdj_itunes::inspect(input, None, &|| false)
            .unwrap()
            .tracks
            .remove(0)
    }
    #[test]
    fn itunes_shared_import_reuses_ids_keeps_missing_and_preserves_tags_through_actual_load() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        let file = crate::test_support::wav_file(1000, 96000);
        let record = source(file.path());
        let path = std::fs::canonicalize(file.path()).unwrap();
        c.library.borrow_mut().insert([path.clone()]);
        let id = c.library.borrow().track_id_for_path(&path).unwrap();
        assert!(c.apply_itunes_record(&record, true).unwrap());
        assert_eq!(c.library.borrow().len(), 1);
        assert_eq!(c.library.borrow().track_id_for_path(&path), Some(id));
        assert!(!c.apply_itunes_record(&record, false).unwrap());
        c.publish_itunes_changes();
        c.load_itunes_track(0, path.clone()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while c.pending.borrow().is_some() {
            c.tick();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(ui.get_left_loaded());
        assert_eq!(ui.get_left_title(), "XML title");
        assert!(!engine.snapshot(DeckId::A).playing);
        assert!(
            c.library
                .borrow()
                .track_by_id(id)
                .unwrap()
                .metadata
                .prefer_imported_tags
        );
        assert_eq!(
            c.library.borrow().track_by_id(id).unwrap().metadata.artist,
            "XML artist"
        );
        engine.play(DeckId::A);
        for _ in 0..128 {
            output.next();
        }
        assert!(c.load_itunes_track(0, path).is_err());
        engine.pause(DeckId::A);
        let dir = tempfile::tempdir().unwrap();
        let missing = source(&dir.path().join("missing.mp3"));
        assert!(c.apply_itunes_record(&missing, true).unwrap());
        assert_eq!(c.library.borrow().len(), 2);
        assert!(c
            .load_itunes_track(1, missing.file.path().unwrap())
            .is_err());
    }
}
