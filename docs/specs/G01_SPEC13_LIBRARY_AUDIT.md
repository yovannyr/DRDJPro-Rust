# G01 – Einzelabgleich Spec 13: Library Management

Quellstand `0f979e3`, statischer Abgleich am 08.11.2026. [Spec 13](13-library-management/spec.md) und [Register](SPEC_REQUIREMENTS_MATRIX.md) enthalten den vollständigen Wortlaut. Der letzte Workspace-Test auf `b783a98` ergab 504 bestandene und sechs ignorierte Tests; nachfolgende Commits änderten Dokumentation. Die Einstufungen sind keine aktuelle UI-, Performance- oder Cloud-Abnahme.

| ID | Status | Nachweis und verbleibende Arbeit |
|---|---|---|
| FR-001 | Teilweise | [crates.rs](../../src/crates.rs) besitzt Erstellen, Umbenennen und Löschen von Crates; ein vollständig gleichwertiger Playlistvertrag mit eigener Reihenfolge ist gesondert nachzuweisen. |
| FR-002 | Teilweise | `add`, `add_many`, `remove` in `crates.rs` und GUI-Aktionen in [app.slint](../../ui/app.slint); Crate-Mitgliedschaft ist nicht automatisch vollständige Playlistsemantik. |
| FR-003 | Offen | Crate-Mitgliedschaften sind Mengen; frei bearbeitbare Trackreihenfolge einer Playlist ist darin nicht nachgewiesen. Prep Queue und Crate nicht gleichsetzen. |
| FR-004 | Teilweise | `TrackSearch` in [library.rs](../../src/library.rs) sucht Titel, Artist, Album, Namen und Pfad; alle geforderten Metadatenfelder sind damit nicht abgedeckt. |
| FR-005 | Offen | Kombinierbare Filter für BPM-Bereich, Key, Genre, Jahr und Rating sind nicht vollständig nachgewiesen. |
| FR-006 | Teilweise | `create_smart` speichert eine Textsuche; eine allgemeine regelbasierte Smart Playlist ist nicht vollständig belegt. |
| FR-007 | Offen | Allgemeine AND-/OR-Regelgruppen und Feldoperatoren fehlen als nachgewiesener Vertrag; Smart-Crate-Textsuche ist dafür kein Nachweis. |
| FR-008 | Teilweise | Verschachtelte Crates mit `create_under`, `ordered_with_depth` und Test `nested_crates_validate_order_and_promote_children_on_delete` vorhanden; Ordner mit vollwertigen Playlists gesondert prüfen. |
| FR-009 | Offen | Vollständiger Editor für alle aufgeführten Metadaten und dauerhafte Overrides ist nicht nachgewiesen. |
| FR-010 | Offen | Benutzerdefinierte Track-Farbmarkierung ist nicht als Bibliotheksfunktion nachgewiesen; Cue-Farbe nicht gleich Trackfarbe. |
| FR-011 | Teilweise | `record_play` in `library.rs` erhöht `play_count` und speichert `last_played_ms`; `playback_history_delta_survives_restart` in [library_store.rs](../../src/library_store.rs) belegt Persistenz als Testpfad. UI, Zählzeitpunkt und Grenzen prüfen. |
| FR-012 | Offen | Listenansicht vorhanden; Umschaltung auf eine eigenständige Grid-Ansicht nicht nachgewiesen. |
| FR-013 | Teilweise | Sortierung vorhandener Spalten in `library.rs`; beliebige Metadatenfelder und beide Richtungen sind nicht pauschal nachgewiesen. |
| FR-014 | Teilweise | `add_many` und Import-/Mehrfachpfade sind vorhanden; konsistente Mehrfachauswahl für alle Batchoperationen ist nicht belegt. |
| FR-015 | Teilweise | Cratezeilen besitzen `count` in `app.slint`; Gesamtdauer einer Playlist ist nicht als vollständiger Anzeigevertrag nachgewiesen. |
| FR-016 | Offen | M3U-/PLS-Export von Playlists ist nicht nachgewiesen. |
| FR-017 | Offen | Import-Deduplikation identischer Pfade ist kein Nachweis einer umfassenden Track-Duplikaterkennung. |

SC-001–008 bleiben offen: definierte Messungen für Suche, Erstellen, Regeln, Ansichtswechsel, Batch und Filter mit 10.000+ Tracks sowie der vollständige Cloud-Sync-Pfad für SC-007 fehlen. Vorhandene Performance-Tests ersetzen keine dokumentierte Messung genau dieser Szenarien.
