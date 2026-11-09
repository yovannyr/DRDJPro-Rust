# Slint-UI-Migration – Arbeitsstand 2026-11-09

**Status: Produktions-UI integriert; strenges Clippy erfolgreich. Gesamttests: 2188 bestanden, 6 ignoriert, 0 fehlgeschlagen. Debug-/Release-Builds erfolgreich; finale Deck-Ladezeiten 2,41 bis 3,98 s einschließlich eines 310-s-Tracks. Visuelle und manuelle Abnahme bleiben auf Nutzerwunsch zurückgestellt.**

## Ausgangspunkt und Umfang

Branch `Layout_Refinement`, Commit `94297ba5612098a666826e2b12368ecb3ad7d2a6`.
Der Checkout wurde nicht aktualisiert. Zahlreiche bereits unversionierte Rust-Dateien
werden von `src/main.rs` eingebunden. Nach ausdrücklich autorisierter Erweiterung
wurden auch deren bestehende Build- und Verhaltensfehler repariert (siehe unten).
`build.rs` kompiliert weiterhin `ui/app.slint`; Manifest, Dependency-Versionen und
Features bleiben unverändert. Startup/Audio-Komposition bleiben im Executable;
`src/lib.rs` teilt nur die Produktions-Slint-Bindings mit Integrationstests.
Kein Commit/Push/Merge.

Die ZIP wurde außerhalb des Checkouts nach
`C:/Users/yrodriguez/AppData/Local/Temp/drdj-layout-reference-20261109/drdjpro-layout`
entpackt. Parser und Installer wurden gelesen. Nur der geprüfte Transformer wurde
zur Erzeugung eines externen Staging-Verzeichnisses ausgeführt; der Installer wurde
nicht ausgeführt. Sein Ergebnis wurde gezielt weiterentwickelt, nicht als geprüfte
Gesamtlösung übernommen. Keine Preview-Rust-Datei oder Cargo-Datei wurde installiert.
QWEN lieferte einen Planungsvorschlag. Dessen Aussagen zu garantierter Kompatibilität
und Vorschläge zum Umgehen von Checks wurden nicht übernommen. Nach Nutzeranweisung
erfolgt die Umsetzung direkt durch GPT-6.1 Sol.

## Komposition und Grenzen

Sieben Module: AppWindow/Workspace in app, Typen/Tokens in theme, gemeinsame
Eingabeelemente in controls, Decks in deck, Waveforms in waveform, Library in library,
EQ/FX in editors. Fonts einschließlich Lizenz liegen in ui/layout-fonts.
Die echte Hauptoberfläche zeigt A/B mit Plattern, Titel, Overview und benannten
Cue-Pads, darunter ausgerichtete Detail-Waveforms, Mixer, Library und Status.
Die gesamte bestehende Library inklusive Paging, Cache, Auswahl, Sortierung,
Suche, Crates und Drag/Drop bleibt in ihren ursprünglichen Komponenten.

`2 DECKS`/`4 SLOTS` ändert ausschließlich `four-decks`. A/B bleiben dieselben
Instanzen mit denselben Bindings und Controlleraktionen. C/D sind separate,
standardmäßig leere Metadatenplätze ohne Transport-Callbacks; ihre Load/Play/Cue/
Sync-Tasten und Scratch sind deaktiviert. Sie lesen keine A/B-Modelle.
C/D besitzen zusätzliche read-only Waveform-Zeilen. Keine Vier-Deck-Engine.
Der Moduswechsel entfernt keine interaktiven A/B-Controls. Bestehende Hold-/Cancel-
Implementierungen wurden übernommen. Deck-Werkzeugwechsel behält seine bestehende
Nudge-/Loop-Jog-Freigabe; Cue-Editor wird bei externem Editierbeginn geöffnet.

Die neue Komposition zeigt den Platter auch in Training als Anzeige; Scratch bleibt
an den bisherigen Controller-/Ansichtsvertrag gebunden. DeckView, Connection-
Defaults und automatische Rückkehr zu Training beim Trennen bleiben erhalten.

Visuelle Abweichungen von der Datenvorschau dienen Funktionserhalt: Die echten
Loop-/Pitch-/Sync-/Nudge-/Cue-Editoren bleiben über die Deck-Tools erreichbar;
Grid-Editor und EQ behalten ihren Workspace-Kontext. Die aktuelle Mindesthöhe
beträgt 860 (zwei Plätze) bzw. 1020 (vier Plätze), jeweils plus 120 für EQ und
110 je Grid-Editor. Die 740/840-Vorgaben wurden nicht erreicht: vorhandene
vollständige Controls benötigen mehr Platz. Native Lesbarkeit/DPI sind offen.

## Historische Prüflage vor der autorisierten Baseline-Reparatur

| Prüfung | Ergebnis | Exitcode |
| --- | --- | --- |
| Baseline `cargo fmt --all -- --check` | Bereits vor UI-Änderung Syntaxfehler in unversionierter `src/automix.rs:403` | 1 |
| Baseline `cargo check --workspace --all-targets --locked` | Cargo.lock passt bereits vor UI-Änderung nicht zum Manifest | 1 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Durch denselben Lockfile-Konflikt blockiert | 1 |
| `cargo test --workspace --locked` | Durch denselben Lockfile-Konflikt blockiert | 1 |
| `cargo build --release --locked` | Durch denselben Lockfile-Konflikt blockiert | 1 |
| Vorhandenes Slint-build-Skript mit externem OUT_DIR | Produktions-UI kompiliert; Import/Re-Export-Fehler des Transformers behoben | 0 |
| Statischer Vergleich aller 481 ursprünglichen AppWindow-Member | Name, Typ/Richtung und Callback-Signatur erhalten | 0 |

Isolierter UI-Test nutzt dieselbe generierte Produktions-UI und die vorhandenen
Slint-1.18.1-Bibliotheken. Das ist kein Ersatz für Cargo-Workspace-Tests oder einen
Start der echten App. Der erste Lauf fand doppelte zugängliche Labels (Button und
Text); der Test selektiert nun ausdrücklich Buttonrollen. Linkerpfad wurde für den
isolierten Aufruf ergänzt, ohne Projektkonfiguration zu ändern.

Unabhängiges Review: GPT-6.1 Sol, Denkaufwand medium. Censor-Indikator im Training
wurde als Befund korrigiert. Finale Test-/Review-Ergebnisse werden unten ergänzt.

## Start und manuelle Abnahme

Die folgenden manuellen Schritte sind ausdrücklich zurückgestellt, bis der Nutzer
sie gemeinsam durchführen möchte. Anschließend gemäß bestehendem App-Startverfahren `cargo run --release --locked` verwenden; keine neue Demo-CLI.
Mit temporärer Testbibliothek prüfen: A/B laden, unabhängig Play/Cue/Scratch,
Moduswechsel während Wiedergabe und bei Cue/Nudge, Loop/Sync/Pitch/Keylock,
alle fünf Tools-Seiten (primäre Aktionen dauerhaft sichtbar), Cue- und Grid-Editor, EQ/FX/Monitor/Sampler/Audio-Panels,
Library-Suche/Sortierung/Crates/Auswahl/Drag/Load, lange Titel/fehlende Metadaten.
C/D müssen leer/read-only bleiben. Fenster 1280×860 und die oben dokumentierten
Minima sowie native DPI-Skalierungen prüfen. Audio-/MC4000-/Hörabnahme und
Performance-/Endurance-Prüfung sind nicht ausgeführt. Kein finaler App-Start und
keine nativen Screenshots des finalen Standes belegt. Ein vorheriger Zwischenstand
wurde mit leeren A/B und 2/4 Plätzen nativ geöffnet und wieder geschlossen;
das belegt weder den finalen Stand noch Audio-/Hardware-Bedienung.

## Sicherer Rückweg

Original der einzigen ersetzten vorhandenen UI-Datei:
`C:/Temp/drdj-layout-original-20261109/app.slint`.
Vor Rücknahme etwaige spätere UI-Änderungen sichern und vergleichen. Nur diese
Datei gezielt nach `ui/app.slint` zurückkopieren. Neue Module und Fonts können
zunächst liegen bleiben (werden dann nicht importiert); keine rekursive Löschung,
kein Reset/Clean und keine Änderung der fremden Rust-Dateien erforderlich.

## Funktions-/Binding-Zuordnung

Die folgende vollständige Liste stammt aus den tatsächlichen Komponenteninstanzen.
Für jeden Eintrag: alter Component-Member → gleichnamiger neuer Member →
bestehende Datenquelle oder Callback-Weiterleitung. `=>` bezeichnet Aktionen,
`<=>` bidirektionale Modelle und `:` eingehende Darstellung. Prüfmethode für
alle Zeilen ist statischer Abgleich plus Slint-Kompilierung; tatsächliche
Controller-/Hardwarewirkung bleibt durch die Gesamtbuild-Blocker unbestätigt.
Gezielte UI-Tests decken Moduszustand, A/B-Transportdispatch und Erreichbarkeit ab.


### CompactDeck DECK A → ui/deck.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `CompactDeck` → `CompactDeck` | `pad-sampler: root.left-pad-sampler; pad-mode => { root.pad-mode(0,true,false); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `platter-mouse-owned: root.left-platter-mouse-owned;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `scratch-pointer(action, x, y) => { root.virtual-platter-pointer(0, action, x, y); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `deck-label: "DECK A"; controller-mode: root.virtual-decks-visible; accent: #36c9ef; track-title: root.left-title; time-text: root.left-time;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `platter-bpm: root.left-platter-bpm; platter-time: root.left-platter-time;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `platter-angle: root.left-platter-angle; platter-touched: root.left-platter-touched;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `censor: root.left-censor;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `external-hover: root.external-drop-deck == 0;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `accepts-drop(data) => { return root.accepts-track-drop(0,data); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `drop-track(data) => { return root.drop-track(0,data); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `waveform: root.left-waveform; colors: root.left-waveform-colors;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `pitch-range-index <=> root.left-pitch-range-index; pitch-min: root.left-pitch-min; pitch-max: root.left-pitch-max; pitch-outside: root.left-pitch-outside; pitch-can-down: root.left-pitch-can-down; pitch-can-up: root.left-pitch-can-up;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `pitch-range => { root.pitch-range-left(); } pitch-down => { root.pitch-down-left(); } pitch-up => { root.pitch-up-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `pitch-percent <=> root.left-pitch-percent; pitch-caption: root.left-pitch-caption; bpm-caption: root.left-bpm-caption;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `nudge(direction, held) => { root.nudge-left(direction, held); } cancel-nudge => { root.cancel-nudge-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `pitch(value) => { root.pitch-left(value); } reset-pitch => { root.reset-pitch-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `can-match-tempo: root.left-can-match-tempo; match-tempo => { root.match-tempo-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `can-phase-sync: root.left-can-phase-sync; phase-sync => { root.phase-sync-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `sync-active: root.left-sync-active; sync-preparing: root.left-sync-preparing; can-sync: root.left-can-sync; toggle-sync => { root.toggle-sync-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `alignment-index <=> root.left-alignment-index; alignment-mode => { root.alignment-mode-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `grid-active: root.left-grid-active; auto-loop-index <=> root.left-auto-loop-index; auto-loop-snap <=> root.left-auto-loop-snap;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `auto-loop => { root.auto-loop-left(); } half-loop => { root.half-loop-left(); } double-loop => { root.double-loop-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `saved-loops: root.left-saved-loops; saved-loop-slot <=> root.left-saved-loop-slot; saved-loop-name <=> root.left-saved-loop-name;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `save-loop => { root.save-loop-left(); } recall-loop => { root.recall-loop-left(); } delete-loop => { root.delete-loop-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `loop-range: root.left-loop-range;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `loop-caption: root.left-loop-caption; loop-has-in: root.left-loop-has-in; loop-ready: root.left-loop-ready; loop-active: root.left-loop-active;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `loop-jog-mode: root.left-loop-jog-mode;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `cancel-loop-jog => { root.controller-loop-jog(0, 0); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `loop-jog => { root.controller-loop-jog(0, root.left-loop-jog-mode == 0 ? 1 : (root.left-loop-jog-mode == 1 ? 2 : 0)); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `loop-in => { root.loop-in-left(); } loop-out => { root.loop-out-left(); } loop-activate => { root.loop-activate-left(); } reloop => { root.reloop-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `hot-cues: root.left-hot-cues;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `quantize-cues <=> root.left-quantize-cues;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `quantize-enabled: root.quantize-enabled;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `quantize-preferences-ready: root.quantize-preferences-ready;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `quantize-triggers <=> root.left-quantize-triggers;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `quantize-step <=> root.left-quantize-step;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `keylock <=> root.left-keylock; keylock-change => { root.keylock-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `cancel-trigger => { root.cancel-trigger-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `cue-editing: root.left-cue-editing; cue-name <=> root.left-cue-name; cue-rgb <=> root.left-cue-rgb;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `edit-hot-cue(slot) => { root.edit-hot-cue-left(slot); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `save-hot-cue => { root.save-hot-cue-left(); } cancel-hot-cue => { root.cancel-hot-cue-left(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `hot-cue(slot, held, input) => { root.hold-hot-cue-left(slot, held, input); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `delete-hot-cue(slot) => { root.delete-hot-cue-left(slot); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `playing: root.left-playing; position <=> root.left-position;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `audio-enabled: root.audio-enabled; loaded: root.left-loaded; loading: root.left-busy; load-busy: root.loading \|\| root.library-busy;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `temporary-cue(held) => { root.temporary-cue(0, held, false); } stutter => { root.stutter(0); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `load => { root.load-left(); } toggle => { root.toggle-left(); } cue => { root.reset-left(); } eject => { root.eject-left(); } seek(value) => { root.seek-left(value); }` | statisch + Slint; Runtime offen |

### CompactDeck DECK B → ui/deck.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `CompactDeck` → `CompactDeck` | `pad-sampler: root.right-pad-sampler; pad-mode => { root.pad-mode(1,true,false); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `platter-mouse-owned: root.right-platter-mouse-owned;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `scratch-pointer(action, x, y) => { root.virtual-platter-pointer(1, action, x, y); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `deck-label: "DECK B"; controller-mode: root.virtual-decks-visible; accent: #ffae58; track-title: root.right-title; time-text: root.right-time;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `platter-bpm: root.right-platter-bpm; platter-time: root.right-platter-time;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `platter-angle: root.right-platter-angle; platter-touched: root.right-platter-touched;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `censor: root.right-censor;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `external-hover: root.external-drop-deck == 1;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `accepts-drop(data) => { return root.accepts-track-drop(1,data); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `drop-track(data) => { return root.drop-track(1,data); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `waveform: root.right-waveform; colors: root.right-waveform-colors;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `pitch-range-index <=> root.right-pitch-range-index; pitch-min: root.right-pitch-min; pitch-max: root.right-pitch-max; pitch-outside: root.right-pitch-outside; pitch-can-down: root.right-pitch-can-down; pitch-can-up: root.right-pitch-can-up;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `pitch-range => { root.pitch-range-right(); } pitch-down => { root.pitch-down-right(); } pitch-up => { root.pitch-up-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `pitch-percent <=> root.right-pitch-percent; pitch-caption: root.right-pitch-caption; bpm-caption: root.right-bpm-caption;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `nudge(direction, held) => { root.nudge-right(direction, held); } cancel-nudge => { root.cancel-nudge-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `pitch(value) => { root.pitch-right(value); } reset-pitch => { root.reset-pitch-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `can-match-tempo: root.right-can-match-tempo; match-tempo => { root.match-tempo-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `can-phase-sync: root.right-can-phase-sync; phase-sync => { root.phase-sync-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `sync-active: root.right-sync-active; sync-preparing: root.right-sync-preparing; can-sync: root.right-can-sync; toggle-sync => { root.toggle-sync-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `alignment-index <=> root.right-alignment-index; alignment-mode => { root.alignment-mode-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `grid-active: root.right-grid-active; auto-loop-index <=> root.right-auto-loop-index; auto-loop-snap <=> root.right-auto-loop-snap;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `auto-loop => { root.auto-loop-right(); } half-loop => { root.half-loop-right(); } double-loop => { root.double-loop-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `saved-loops: root.right-saved-loops; saved-loop-slot <=> root.right-saved-loop-slot; saved-loop-name <=> root.right-saved-loop-name;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `save-loop => { root.save-loop-right(); } recall-loop => { root.recall-loop-right(); } delete-loop => { root.delete-loop-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `loop-range: root.right-loop-range;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `loop-caption: root.right-loop-caption; loop-has-in: root.right-loop-has-in; loop-ready: root.right-loop-ready; loop-active: root.right-loop-active;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `loop-jog-mode: root.right-loop-jog-mode;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `cancel-loop-jog => { root.controller-loop-jog(1, 0); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `loop-jog => { root.controller-loop-jog(1, root.right-loop-jog-mode == 0 ? 1 : (root.right-loop-jog-mode == 1 ? 2 : 0)); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `loop-in => { root.loop-in-right(); } loop-out => { root.loop-out-right(); } loop-activate => { root.loop-activate-right(); } reloop => { root.reloop-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `hot-cues: root.right-hot-cues;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `quantize-cues <=> root.right-quantize-cues;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `quantize-enabled: root.quantize-enabled;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `quantize-preferences-ready: root.quantize-preferences-ready;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `quantize-triggers <=> root.right-quantize-triggers;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `quantize-step <=> root.right-quantize-step;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `keylock <=> root.right-keylock; keylock-change => { root.keylock-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `cancel-trigger => { root.cancel-trigger-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `cue-editing: root.right-cue-editing; cue-name <=> root.right-cue-name; cue-rgb <=> root.right-cue-rgb;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `edit-hot-cue(slot) => { root.edit-hot-cue-right(slot); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `save-hot-cue => { root.save-hot-cue-right(); } cancel-hot-cue => { root.cancel-hot-cue-right(); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `hot-cue(slot, held, input) => { root.hold-hot-cue-right(slot, held, input); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `delete-hot-cue(slot) => { root.delete-hot-cue-right(slot); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `playing: root.right-playing; position <=> root.right-position;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `audio-enabled: root.audio-enabled; loaded: root.right-loaded; loading: root.right-busy; load-busy: root.loading \|\| root.library-busy;` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `temporary-cue(held) => { root.temporary-cue(1, held, false); } stutter => { root.stutter(1); }` | statisch + Slint; Runtime offen |
| `CompactDeck` → `CompactDeck` | `load => { root.load-right(); } toggle => { root.toggle-right(); } cue => { root.reset-right(); } eject => { root.eject-right(); } seek(value) => { root.seek-right(value); }` | statisch + Slint; Runtime offen |

### ChannelMeter A → ui/controls.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `ChannelMeter` → `ChannelMeter` | `deck: "A"; caption: root.channel-a-peak-caption; level: root.channel-a-peak-level; clipped: root.channel-a-clipped; warning: root.channel-a-peak-warning; clear => { root.clear-channel-clip(0); }` | statisch + Slint; Runtime offen |

### ChannelMeter B → ui/controls.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `ChannelMeter` → `ChannelMeter` | `deck: "B"; caption: root.channel-b-peak-caption; level: root.channel-b-peak-level; clipped: root.channel-b-clipped; warning: root.channel-b-peak-warning; clear => { root.clear-channel-clip(1); }` | statisch + Slint; Runtime offen |

### EqStrip A → ui/editors.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `EqStrip` → `EqStrip` | `deck: "A"; trim <=> root.eq-trim-a; low <=> root.eq-low-a; mid <=> root.eq-mid-a; high <=> root.eq-high-a; filter <=> root.eq-filter-a; changed => { root.eq-changed(0); }` | statisch + Slint; Runtime offen |

### EqStrip B → ui/editors.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `EqStrip` → `EqStrip` | `deck: "B"; trim <=> root.eq-trim-b; low <=> root.eq-low-b; mid <=> root.eq-mid-b; high <=> root.eq-high-b; filter <=> root.eq-filter-b; changed => { root.eq-changed(1); }` | statisch + Slint; Runtime offen |

### DetailWaveform DECK A → ui/waveform.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `DetailWaveform` → `DetailWaveform` | `deck-label: "DECK A"; accent: #36c9ef; detail: root.left-detail; colors: root.left-detail-colors;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `loop-range: root.left-detail-loop-range;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `hot-cues: root.left-hot-cues; beats: root.left-beats; position: root.left-position; marker: root.left-detail-marker;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `detail-start: root.left-detail-start; detail-end: root.left-detail-end;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `detail-window <=> root.left-detail-window; grid-bpm <=> root.left-grid-bpm;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-active: root.left-grid-active; grid-caption: root.left-grid-caption;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-editing: root.left-grid-editing; grid-draft-bpm <=> root.left-grid-draft-bpm; grid-draft-beats <=> root.left-grid-draft-beats; grid-draft-denominator-index <=> root.left-grid-draft-denominator-index;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-meter-bar <=> root.left-grid-meter-bar; grid-meter-beats <=> root.left-grid-meter-beats; grid-meter-denominator-index <=> root.left-grid-meter-denominator-index; grid-meter-summary: root.left-grid-meter-summary;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `meter-set => { root.grid-meter-set-left(); } meter-remove => { root.grid-meter-remove-left(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-tempo-beat <=> root.left-grid-tempo-beat; grid-tempo-bpm <=> root.left-grid-tempo-bpm; grid-tempo-summary: root.left-grid-tempo-summary;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `tempo-set => { root.grid-tempo-set-left(); } tempo-remove => { root.grid-tempo-remove-left(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-warp-beat <=> root.left-grid-warp-beat; grid-warp-summary: root.left-grid-warp-summary;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `warp-set => { root.grid-warp-set-left(); } warp-remove => { root.grid-warp-remove-left(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-controls-enabled: root.audio-enabled && root.left-loaded && !root.left-busy;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `edit-bpm => { root.edit-grid-bpm-left(); } anchor-grid => { root.grid-anchor-left(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `earlier-grid => { root.grid-earlier-left(); } later-grid => { root.grid-later-left(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `save-grid => { root.save-grid-left(); } cancel-grid => { root.cancel-grid-left(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `suggested-bpm: root.left-suggested-bpm; enabled: root.audio-enabled && root.left-loaded;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `seek(value) => { root.seek-left(value); } zoom => { root.zoom-left(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `set-grid => { root.grid-left(); } clear-grid => { root.clear-grid-left(); }` | statisch + Slint; Runtime offen |

### DetailWaveform DECK B → ui/waveform.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `DetailWaveform` → `DetailWaveform` | `deck-label: "DECK B"; accent: #ffae58; detail: root.right-detail; colors: root.right-detail-colors;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `loop-range: root.right-detail-loop-range;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `hot-cues: root.right-hot-cues; beats: root.right-beats; position: root.right-position; marker: root.right-detail-marker;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `detail-start: root.right-detail-start; detail-end: root.right-detail-end;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `detail-window <=> root.right-detail-window; grid-bpm <=> root.right-grid-bpm;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-active: root.right-grid-active; grid-caption: root.right-grid-caption;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-editing: root.right-grid-editing; grid-draft-bpm <=> root.right-grid-draft-bpm; grid-draft-beats <=> root.right-grid-draft-beats; grid-draft-denominator-index <=> root.right-grid-draft-denominator-index;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-meter-bar <=> root.right-grid-meter-bar; grid-meter-beats <=> root.right-grid-meter-beats; grid-meter-denominator-index <=> root.right-grid-meter-denominator-index; grid-meter-summary: root.right-grid-meter-summary;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `meter-set => { root.grid-meter-set-right(); } meter-remove => { root.grid-meter-remove-right(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-tempo-beat <=> root.right-grid-tempo-beat; grid-tempo-bpm <=> root.right-grid-tempo-bpm; grid-tempo-summary: root.right-grid-tempo-summary;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `tempo-set => { root.grid-tempo-set-right(); } tempo-remove => { root.grid-tempo-remove-right(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-warp-beat <=> root.right-grid-warp-beat; grid-warp-summary: root.right-grid-warp-summary;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `warp-set => { root.grid-warp-set-right(); } warp-remove => { root.grid-warp-remove-right(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `grid-controls-enabled: root.audio-enabled && root.right-loaded && !root.right-busy;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `edit-bpm => { root.edit-grid-bpm-right(); } anchor-grid => { root.grid-anchor-right(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `earlier-grid => { root.grid-earlier-right(); } later-grid => { root.grid-later-right(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `save-grid => { root.save-grid-right(); } cancel-grid => { root.cancel-grid-right(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `suggested-bpm: root.right-suggested-bpm; enabled: root.audio-enabled && root.right-loaded;` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `seek(value) => { root.seek-right(value); } zoom => { root.zoom-right(); }` | statisch + Slint; Runtime offen |
| `DetailWaveform` → `DetailWaveform` | `set-grid => { root.grid-right(); } clear-grid => { root.clear-grid-right(); }` | statisch + Slint; Runtime offen |

### CratePanel CratePanel → ui/library.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `CratePanel` → `CratePanel` | `managing: root.library-tab == 2;` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `accessible-role: groupbox; accessible-label: "Crate navigation"; width: 190px;` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `rows: root.crate-rows; names: root.crate-names; selection: root.selected-crate;` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `history-selected: root.selected-crate == -1;` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `busy: root.loading \|\| root.library-busy \|\| !root.audio-enabled;` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `has-track: root.selected-track >= 0;` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `selected-smart: root.selected-crate-smart;` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `target-index <=> root.target-crate-index;` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `editor-open: root.crate-editor-open; delete-pending: root.crate-delete-pending;` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `dialog-title: root.crate-dialog-title; editor-smart: root.crate-editor-smart;` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `edit-name <=> root.crate-edit-name; edit-smart-query <=> root.crate-edit-smart-query;` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `select-crate(id) => { root.select-crate(id); } target-changed(index) => { root.target-crate(index); }` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `new-crate => { root.new-crate(); } new-smart-crate => { root.new-smart-crate(); } new-subcrate => { root.new-subcrate(); } rename-crate => { root.rename-crate(); } delete-crate => { root.delete-crate(); }` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `save-name => { root.save-crate-name(); } cancel-dialog => { root.cancel-crate-dialog(); } confirm-delete => { root.confirm-delete-crate(); }` | statisch + Slint; Runtime offen |
| `CratePanel` → `CratePanel` | `add-track => { root.add-to-crate(); } remove-track => { root.remove-from-crate(); }` | statisch + Slint; Runtime offen |

### TrackLibrary track-library → ui/library.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `TrackLibrary` → `TrackLibrary` | `columns: root.table-columns; all-columns: root.all-table-columns; table-width: root.table-width;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `table-scroll-x <=> root.table-scroll-x;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `sort-field: root.table-sort-field; sort-descending: root.table-sort-descending; columns-enabled: root.table-columns-enabled;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `change-column(id, action) => { root.change-table-column(id, action); }` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `analysis-title: root.analysis-title; analysis-summary: root.analysis-summary; analysis-waveform: root.analysis-waveform;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `analysis-progress: root.analysis-progress;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `analysis-running: root.analysis-running;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `can-analyze: root.library-has-analysis-targets && !root.loading && !root.library-busy;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `analyze => { root.analyze-library(); }` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `tab <=> root.library-tab;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `prep-action(action) => { root.prep-action(action); }` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `prep-queue-count: root.prep-queue-count;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `tab-selected(index) => { root.library-tab-selected(index); }` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `horizontal-stretch: 1; min-width: 0px;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `rows: root.library-rows; count-text: root.library-count; empty-text: root.library-empty;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `page-text: root.library-page-text; can-previous-page: root.library-can-previous-page; can-next-page: root.library-can-next-page;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `change-page(delta) => { root.library-change-page(delta); }` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `storage-text: root.library-storage; save-failed: root.library-save-failed;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `selection: root.selected-track; query <=> root.library-query;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `track-transfer(path) => { return root.track-transfer(path); }` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `busy: root.loading \|\| root.library-busy \|\| !root.audio-enabled;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `can-relocate: root.library-can-relocate-selected;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `can-load-left: root.audio-enabled && root.selected-track >= 0 && !root.loading && !root.library-busy && !root.left-playing;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `can-load-right: root.audio-enabled && root.selected-track >= 0 && !root.loading && !root.library-busy && !root.right-playing;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `import-files => { root.import-library(); } import-folder => { root.import-library-folder(); } search => { root.library-search(); }` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `relocate-track => { root.relocate-library-track(); }` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `select-track(id) => { root.select-library(id); } move-selection(delta) => { root.move-library-selection(delta); }` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `sort-column(field) => { root.sort-library(field); }` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `load-left => { root.load-library-left(); } load-right => { root.load-library-right(); }` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `job-active: root.background-job-active;` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `cancel-job => {root.cancel-background-job();}` | statisch + Slint; Runtime offen |
| `TrackLibrary` → `TrackLibrary` | `retry-save => { root.retry-library-save(); }` | statisch + Slint; Runtime offen |

### FxStrip A → ui/editors.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `FxStrip` → `FxStrip` | `deck:"A";summary:root.fx-summary-a;selected-slot <=> root.fx-selected-a;effect-kind <=> root.fx-kind-a;legacy <=> root.fx-legacy-a;slot-action(slot,action,value)=>{root.fx-slot-action(0,slot,action,value);}legacy-changed(value)=>{root.fx-legacy-changed(0,value);}time-base <=> root.fx-base-a;division <=> root.fx-division-a;timing:root.fx-timing-a;timing-changed(base,division)=>{root.fx-timing-changed(0,base,division);}tap(held)=>{root.fx-tap(0,held);}tap-cancel=>{root.fx-tap-cancel(0);}ready:root.quantize-preferences-ready;active <=> root.fx-active-a;time <=> root.fx-time-a;feedback <=> root.fx-feedback-a;wet <=> root.fx-wet-a;slot <=> root.fx-slot-a;changed=>{root.fx-changed(0);}preset(slot,save)=>{root.fx-preset(0,slot,save);}` | statisch + Slint; Runtime offen |

### FxStrip B → ui/editors.slint

| Altes Control/Member → neues Control/Member | Datenquelle / Callback | Prüfmethode |
| --- | --- | --- |
| `FxStrip` → `FxStrip` | `deck:"B";summary:root.fx-summary-b;selected-slot <=> root.fx-selected-b;effect-kind <=> root.fx-kind-b;legacy <=> root.fx-legacy-b;slot-action(slot,action,value)=>{root.fx-slot-action(1,slot,action,value);}legacy-changed(value)=>{root.fx-legacy-changed(1,value);}time-base <=> root.fx-base-b;division <=> root.fx-division-b;timing:root.fx-timing-b;timing-changed(base,division)=>{root.fx-timing-changed(1,base,division);}tap(held)=>{root.fx-tap(1,held);}tap-cancel=>{root.fx-tap-cancel(1);}ready:root.quantize-preferences-ready;active <=> root.fx-active-b;time <=> root.fx-time-b;feedback <=> root.fx-feedback-b;wet <=> root.fx-wet-b;slot <=> root.fx-slot-b;changed=>{root.fx-changed(1);}preset(slot,save)=>{root.fx-preset(1,slot,save);}` | statisch + Slint; Runtime offen |

## Vollständiger öffentlicher Vertrag

| Member | Art, Richtung / Parameter und Typ |
| --- | --- |
| `left-platter-mouse-owned` | `('property', 'in', 'bool')` |
| `right-platter-mouse-owned` | `('property', 'in', 'bool')` |
| `left-loop-jog-mode` | `('property', 'in', 'int')` |
| `right-loop-jog-mode` | `('property', 'in', 'int')` |
| `left-censor` | `('property', 'in', 'bool')` |
| `right-censor` | `('property', 'in', 'bool')` |
| `left-pad-sampler` | `('property', 'in', 'bool')` |
| `right-pad-sampler` | `('property', 'in', 'bool')` |
| `controller-browser-crates` | `('property', 'in-out', 'bool')` |
| `controller-editor-open` | `('property', 'out', 'bool')` |
| `controller-search-focused` | `('property', 'out', 'bool')` |
| `controller-shift` | `('property', 'in-out', 'bool')` |
| `left-platter-angle` | `('property', 'in', 'float')` |
| `right-platter-angle` | `('property', 'in', 'float')` |
| `left-platter-touched` | `('property', 'in', 'bool')` |
| `right-platter-touched` | `('property', 'in', 'bool')` |
| `left-vinyl` | `('property', 'in', 'bool')` |
| `right-vinyl` | `('property', 'in', 'bool')` |
| `table-columns` | `('property', 'in', '[TableColumn]')` |
| `all-table-columns` | `('property', 'in', '[TableColumn]')` |
| `table-width` | `('property', 'in', 'float')` |
| `table-sort-field` | `('property', 'in', 'int')` |
| `table-sort-descending` | `('property', 'in', 'bool')` |
| `table-columns-enabled` | `('property', 'in', 'bool')` |
| `table-scroll-x` | `('property', 'in-out', 'length')` |
| `external-drop-deck` | `('property', 'in', 'int')` |
| `library-performance` | `('property', 'in', 'string')` |
| `active-hot-cue-key` | `('property', 'in-out', 'int')` |
| `left-waveform` | `('property', 'in', '[float]')` |
| `right-waveform` | `('property', 'in', '[float]')` |
| `left-detail` | `('property', 'in', '[float]')` |
| `right-detail` | `('property', 'in', '[float]')` |
| `left-waveform-colors` | `('property', 'in', '[color]')` |
| `right-waveform-colors` | `('property', 'in', '[color]')` |
| `left-detail-colors` | `('property', 'in', '[color]')` |
| `right-detail-colors` | `('property', 'in', '[color]')` |
| `left-beats` | `('property', 'in', '[BeatLine]')` |
| `right-beats` | `('property', 'in', '[BeatLine]')` |
| `left-grid-active` | `('property', 'in', 'bool')` |
| `right-grid-active` | `('property', 'in', 'bool')` |
| `left-grid-caption` | `('property', 'in', 'string')` |
| `right-grid-caption` | `('property', 'in', 'string')` |
| `left-grid-bpm` | `('property', 'in-out', 'int')` |
| `right-grid-bpm` | `('property', 'in-out', 'int')` |
| `left-suggested-bpm` | `('property', 'in', 'int')` |
| `right-suggested-bpm` | `('property', 'in', 'int')` |
| `left-detail-start` | `('property', 'in', 'float')` |
| `left-detail-end` | `('property', 'in', 'float')` |
| `left-detail-marker` | `('property', 'in', 'float')` |
| `right-detail-start` | `('property', 'in', 'float')` |
| `right-detail-end` | `('property', 'in', 'float')` |
| `right-detail-marker` | `('property', 'in', 'float')` |
| `left-detail-window` | `('property', 'in-out', 'int')` |
| `right-detail-window` | `('property', 'in-out', 'int')` |
| `audio-enabled` | `('property', 'in', 'bool')` |
| `output-pair-options` | `('property', 'in', '[string]')` |
| `output-profile-status` | `('property', 'in', 'string')` |
| `output-pair-index` | `('property', 'in-out', 'int')` |
| `quantize-enabled` | `('property', 'in-out', 'bool')` |
| `quantize-preferences-ready` | `('property', 'in', 'bool')` |
| `loading` | `('property', 'in', 'bool')` |
| `background-job-active` | `('property', 'in', 'bool')` |
| `left-busy` | `('property', 'in', 'bool')` |
| `right-busy` | `('property', 'in', 'bool')` |
| `left-loaded` | `('property', 'in', 'bool')` |
| `right-loaded` | `('property', 'in', 'bool')` |
| `left-title` | `('property', 'in', 'string')` |
| `right-title` | `('property', 'in', 'string')` |
| `left-time` | `('property', 'in', 'string')` |
| `right-time` | `('property', 'in', 'string')` |
| `status` | `('property', 'in', 'string')` |
| `controller-status` | `('property', 'in', 'string')` |
| `mc4000-connected` | `('property', 'in', 'bool')` |
| `deck-view-index` | `('property', 'in-out', 'int')` |
| `deck-view` | `('property', 'out', 'DeckView')` |
| `virtual-decks-visible` | `('property', 'out', 'bool')` |
| `left-platter-bpm` | `('property', 'in', 'string')` |
| `right-platter-bpm` | `('property', 'in', 'string')` |
| `left-platter-time` | `('property', 'in', 'string')` |
| `right-platter-time` | `('property', 'in', 'string')` |
| `controller-reconnect-requested` | `('property', 'in-out', 'bool')` |
| `output-peak-caption` | `('property', 'in', 'string')` |
| `output-peak-level` | `('property', 'in', 'float')` |
| `output-peak-warning` | `('property', 'in', 'bool')` |
| `channel-a` | `('property', 'in-out', 'float')` |
| `channel-b` | `('property', 'in-out', 'float')` |
| `crossfader` | `('property', 'in-out', 'float')` |
| `eq-open` | `('property', 'in-out', 'bool')` |
| `eq-trim-a` | `('property', 'in-out', 'float')` |
| `eq-low-a` | `('property', 'in-out', 'float')` |
| `eq-mid-a` | `('property', 'in-out', 'float')` |
| `eq-high-a` | `('property', 'in-out', 'float')` |
| `eq-filter-a` | `('property', 'in-out', 'float')` |
| `eq-trim-b` | `('property', 'in-out', 'float')` |
| `eq-low-b` | `('property', 'in-out', 'float')` |
| `eq-mid-b` | `('property', 'in-out', 'float')` |
| `eq-high-b` | `('property', 'in-out', 'float')` |
| `eq-filter-b` | `('property', 'in-out', 'float')` |
| `fx-open` | `('property', 'in-out', 'bool')` |
| `fx-active-a` | `('property', 'in-out', 'bool')` |
| `fx-active-b` | `('property', 'in-out', 'bool')` |
| `fx-time-a` | `('property', 'in-out', 'float')` |
| `fx-time-b` | `('property', 'in-out', 'float')` |
| `fx-feedback-a` | `('property', 'in-out', 'float')` |
| `fx-feedback-b` | `('property', 'in-out', 'float')` |
| `fx-wet-a` | `('property', 'in-out', 'float')` |
| `fx-wet-b` | `('property', 'in-out', 'float')` |
| `fx-slot-a` | `('property', 'in-out', 'int')` |
| `fx-slot-b` | `('property', 'in-out', 'int')` |
| `fx-status` | `('property', 'in', 'string')` |
| `fx-base-a` | `('property', 'in-out', 'int')` |
| `fx-base-b` | `('property', 'in-out', 'int')` |
| `fx-division-a` | `('property', 'in-out', 'int')` |
| `fx-division-b` | `('property', 'in-out', 'int')` |
| `fx-timing-a` | `('property', 'in', 'string')` |
| `fx-timing-b` | `('property', 'in', 'string')` |
| `fx-time-available-a` | `('property', 'in', 'bool')` |
| `fx-time-available-b` | `('property', 'in', 'bool')` |
| `fx-chain-ready` | `('property', 'in', 'bool')` |
| `sampler-open` | `('property', 'in-out', 'bool')` |
| `sampler-ready` | `('property', 'in', 'bool')` |
| `sampler-loading` | `('property', 'in', 'bool')` |
| `sampler-slots` | `('property', 'in', '[SamplerRow]')` |
| `sampler-gain` | `('property', 'in-out', 'float')` |
| `sampler-status` | `('property', 'in', 'string')` |
| `fx-summary-a` | `('property', 'in', 'string')` |
| `fx-summary-b` | `('property', 'in', 'string')` |
| `fx-selected-a` | `('property', 'in-out', 'int')` |
| `fx-selected-b` | `('property', 'in-out', 'int')` |
| `fx-kind-a` | `('property', 'in-out', 'int')` |
| `fx-kind-b` | `('property', 'in-out', 'int')` |
| `fx-legacy-a` | `('property', 'in-out', 'bool')` |
| `fx-legacy-b` | `('property', 'in-out', 'bool')` |
| `fx-slot1-a` | `('property', 'in', 'bool')` |
| `fx-slot2-a` | `('property', 'in', 'bool')` |
| `fx-slot3-a` | `('property', 'in', 'bool')` |
| `fx-slot1-b` | `('property', 'in', 'bool')` |
| `fx-slot2-b` | `('property', 'in', 'bool')` |
| `fx-slot3-b` | `('property', 'in', 'bool')` |
| `monitor-open` | `('property', 'in-out', 'bool')` |
| `monitor-cue-a` | `('property', 'in-out', 'bool')` |
| `monitor-cue-b` | `('property', 'in-out', 'bool')` |
| `monitor-available` | `('property', 'in', 'bool')` |
| `monitor-status` | `('property', 'in', 'string')` |
| `monitor-gain` | `('property', 'in-out', 'float')` |
| `monitor-mix` | `('property', 'in-out', 'float')` |
| `monitor-split` | `('property', 'in-out', 'bool')` |
| `controller-meter-live` | `('property', 'in', 'bool')` |
| `controller-master-peak-level` | `('property', 'in', 'float')` |
| `controller-master-peak-warning` | `('property', 'in', 'bool')` |
| `audio-device-open` | `('property', 'in-out', 'bool')` |
| `audio-device-names` | `('property', 'in', '[string]')` |
| `audio-device-choice` | `('property', 'in-out', 'int')` |
| `audio-device-scanning` | `('property', 'in', 'bool')` |
| `audio-device-saving` | `('property', 'in', 'bool')` |
| `audio-device-ready` | `('property', 'in', 'bool')` |
| `audio-device-current` | `('property', 'in', 'string')` |
| `audio-device-status` | `('property', 'in', 'string')` |
| `audio-startup-warning` | `('property', 'in', 'string')` |
| `audio-device-switching` | `('property', 'in', 'bool')` |
| `channel-a-peak-caption` | `('property', 'in', 'string')` |
| `channel-b-peak-caption` | `('property', 'in', 'string')` |
| `channel-a-peak-level` | `('property', 'in', 'float')` |
| `channel-b-peak-level` | `('property', 'in', 'float')` |
| `channel-a-clipped` | `('property', 'in', 'bool')` |
| `channel-b-clipped` | `('property', 'in', 'bool')` |
| `channel-a-peak-warning` | `('property', 'in', 'bool')` |
| `channel-b-peak-warning` | `('property', 'in', 'bool')` |
| `left-playing` | `('property', 'in-out', 'bool')` |
| `right-playing` | `('property', 'in-out', 'bool')` |
| `left-position` | `('property', 'in-out', 'float')` |
| `right-position` | `('property', 'in-out', 'float')` |
| `left-hot-cues` | `('property', 'in', '[HotCueRow]')` |
| `left-quantize-cues` | `('property', 'in-out', 'bool')` |
| `left-quantize-triggers` | `('property', 'in-out', 'bool')` |
| `left-quantize-step` | `('property', 'in-out', 'int')` |
| `left-keylock` | `('property', 'in-out', 'bool')` |
| `left-pitch-range-index` | `('property', 'in-out', 'int')` |
| `left-pitch-min` | `('property', 'in', 'float')` |
| `left-pitch-max` | `('property', 'in', 'float')` |
| `left-pitch-outside` | `('property', 'in', 'bool')` |
| `left-pitch-can-down` | `('property', 'in', 'bool')` |
| `left-pitch-can-up` | `('property', 'in', 'bool')` |
| `left-pitch-percent` | `('property', 'in-out', 'float')` |
| `left-pitch-caption` | `('property', 'in', 'string')` |
| `left-bpm-caption` | `('property', 'in', 'string')` |
| `left-can-match-tempo` | `('property', 'in', 'bool')` |
| `left-can-phase-sync` | `('property', 'in', 'bool')` |
| `left-sync-active` | `('property', 'in', 'bool')` |
| `left-sync-preparing` | `('property', 'in', 'bool')` |
| `left-can-sync` | `('property', 'in', 'bool')` |
| `left-alignment-index` | `('property', 'in-out', 'int')` |
| `left-auto-loop-snap` | `('property', 'in-out', 'bool')` |
| `left-auto-loop-index` | `('property', 'in-out', 'int')` |
| `left-saved-loops` | `('property', 'in', '[SavedLoopRow]')` |
| `left-saved-loop-slot` | `('property', 'in-out', 'int')` |
| `left-saved-loop-name` | `('property', 'in-out', 'string')` |
| `left-loop-range` | `('property', 'in', 'LoopRange')` |
| `left-detail-loop-range` | `('property', 'in', 'LoopRange')` |
| `left-loop-caption` | `('property', 'in', 'string')` |
| `left-loop-has-in` | `('property', 'in', 'bool')` |
| `left-loop-ready` | `('property', 'in', 'bool')` |
| `left-loop-active` | `('property', 'in', 'bool')` |
| `left-cue-editing` | `('property', 'in', 'bool')` |
| `left-cue-name` | `('property', 'in-out', 'string')` |
| `left-cue-rgb` | `('property', 'in-out', 'string')` |
| `right-hot-cues` | `('property', 'in', '[HotCueRow]')` |
| `right-quantize-cues` | `('property', 'in-out', 'bool')` |
| `right-quantize-triggers` | `('property', 'in-out', 'bool')` |
| `right-quantize-step` | `('property', 'in-out', 'int')` |
| `right-keylock` | `('property', 'in-out', 'bool')` |
| `right-pitch-range-index` | `('property', 'in-out', 'int')` |
| `right-pitch-min` | `('property', 'in', 'float')` |
| `right-pitch-max` | `('property', 'in', 'float')` |
| `right-pitch-outside` | `('property', 'in', 'bool')` |
| `right-pitch-can-down` | `('property', 'in', 'bool')` |
| `right-pitch-can-up` | `('property', 'in', 'bool')` |
| `right-pitch-percent` | `('property', 'in-out', 'float')` |
| `right-pitch-caption` | `('property', 'in', 'string')` |
| `right-bpm-caption` | `('property', 'in', 'string')` |
| `right-can-match-tempo` | `('property', 'in', 'bool')` |
| `right-can-phase-sync` | `('property', 'in', 'bool')` |
| `right-sync-active` | `('property', 'in', 'bool')` |
| `right-sync-preparing` | `('property', 'in', 'bool')` |
| `right-can-sync` | `('property', 'in', 'bool')` |
| `right-alignment-index` | `('property', 'in-out', 'int')` |
| `right-auto-loop-snap` | `('property', 'in-out', 'bool')` |
| `right-auto-loop-index` | `('property', 'in-out', 'int')` |
| `right-saved-loops` | `('property', 'in', '[SavedLoopRow]')` |
| `right-saved-loop-slot` | `('property', 'in-out', 'int')` |
| `right-saved-loop-name` | `('property', 'in-out', 'string')` |
| `right-loop-range` | `('property', 'in', 'LoopRange')` |
| `right-detail-loop-range` | `('property', 'in', 'LoopRange')` |
| `right-loop-caption` | `('property', 'in', 'string')` |
| `right-loop-has-in` | `('property', 'in', 'bool')` |
| `right-loop-ready` | `('property', 'in', 'bool')` |
| `right-loop-active` | `('property', 'in', 'bool')` |
| `right-cue-editing` | `('property', 'in', 'bool')` |
| `right-cue-name` | `('property', 'in-out', 'string')` |
| `right-cue-rgb` | `('property', 'in-out', 'string')` |
| `left-grid-editing` | `('property', 'in', 'bool')` |
| `right-grid-editing` | `('property', 'in', 'bool')` |
| `left-grid-draft-bpm` | `('property', 'in-out', 'string')` |
| `left-grid-draft-beats` | `('property', 'in-out', 'int')` |
| `left-grid-draft-denominator-index` | `('property', 'in-out', 'int')` |
| `left-grid-meter-bar` | `('property', 'in-out', 'int')` |
| `left-grid-meter-beats` | `('property', 'in-out', 'int')` |
| `left-grid-meter-denominator-index` | `('property', 'in-out', 'int')` |
| `left-grid-meter-summary` | `('property', 'in', 'string')` |
| `left-grid-tempo-beat` | `('property', 'in-out', 'int')` |
| `left-grid-tempo-bpm` | `('property', 'in-out', 'string')` |
| `left-grid-tempo-summary` | `('property', 'in', 'string')` |
| `left-grid-warp-beat` | `('property', 'in-out', 'int')` |
| `left-grid-warp-summary` | `('property', 'in', 'string')` |
| `right-grid-draft-bpm` | `('property', 'in-out', 'string')` |
| `right-grid-draft-beats` | `('property', 'in-out', 'int')` |
| `right-grid-draft-denominator-index` | `('property', 'in-out', 'int')` |
| `right-grid-meter-bar` | `('property', 'in-out', 'int')` |
| `right-grid-meter-beats` | `('property', 'in-out', 'int')` |
| `right-grid-meter-denominator-index` | `('property', 'in-out', 'int')` |
| `right-grid-meter-summary` | `('property', 'in', 'string')` |
| `right-grid-tempo-beat` | `('property', 'in-out', 'int')` |
| `right-grid-tempo-bpm` | `('property', 'in-out', 'string')` |
| `right-grid-tempo-summary` | `('property', 'in', 'string')` |
| `right-grid-warp-beat` | `('property', 'in-out', 'int')` |
| `right-grid-warp-summary` | `('property', 'in', 'string')` |
| `library-rows` | `('property', 'in', '[LibraryRow]')` |
| `library-tab` | `('property', 'in-out', 'int')` |
| `analysis-title` | `('property', 'in', 'string')` |
| `analysis-summary` | `('property', 'in', 'string')` |
| `analysis-progress` | `('property', 'in', 'string')` |
| `library-has-analysis-targets` | `('property', 'in', 'bool')` |
| `analysis-running` | `('property', 'in', 'bool')` |
| `analysis-waveform` | `('property', 'in', '[float]')` |
| `prep-queue-count` | `('property', 'in', 'int')` |
| `library-count` | `('property', 'in', 'string')` |
| `library-page-text` | `('property', 'in', 'string')` |
| `library-can-previous-page` | `('property', 'in', 'bool')` |
| `library-can-next-page` | `('property', 'in', 'bool')` |
| `library-scroll-y` | `('property', 'out', 'length')` |
| `library-storage` | `('property', 'in', 'string')` |
| `library-save-failed` | `('property', 'in', 'bool')` |
| `library-empty` | `('property', 'in', 'string')` |
| `library-busy` | `('property', 'in', 'bool')` |
| `selected-track` | `('property', 'in', 'int')` |
| `library-can-relocate-selected` | `('property', 'in', 'bool')` |
| `library-query` | `('property', 'in-out', 'string')` |
| `crate-rows` | `('property', 'in', '[CrateRow]')` |
| `crate-names` | `('property', 'in', '[string]')` |
| `selected-crate` | `('property', 'in', 'int')` |
| `selected-crate-smart` | `('property', 'in', 'bool')` |
| `target-crate-index` | `('property', 'in-out', 'int')` |
| `crate-editor-open` | `('property', 'in', 'bool')` |
| `crate-delete-pending` | `('property', 'in', 'bool')` |
| `crate-dialog-title` | `('property', 'in', 'string')` |
| `crate-edit-name` | `('property', 'in-out', 'string')` |
| `crate-editor-smart` | `('property', 'in', 'bool')` |
| `crate-edit-smart-query` | `('property', 'in-out', 'string')` |
| `virtual-platter-pointer` | `('callback', False, 'int,int,float,float', 'void')` |
| `controller-jog` | `('callback', False, 'int,int', 'void')` |
| `controller-seek-hold` | `('callback', False, 'int,int,bool', 'void')` |
| `controller-navigation-shift` | `('callback', False, 'bool', 'void')` |
| `controller-loop-boundary` | `('callback', False, 'int,int', 'void')` |
| `controller-loop-jog` | `('callback', False, 'int,int', 'void')` |
| `controller-platter-touch` | `('callback', False, 'int,bool', 'void')` |
| `controller-vinyl` | `('callback', False, 'int', 'void')` |
| `controller-censor` | `('callback', False, 'int,bool', 'void')` |
| `controller-pitch-fader` | `('callback', False, 'int,int', 'void')` |
| `controller-pitch-range-left` | `('callback', False, '', 'void')` |
| `controller-pitch-range-right` | `('callback', False, '', 'void')` |
| `controller-bend-left` | `('callback', False, 'int,bool', 'void')` |
| `controller-bend-right` | `('callback', False, 'int,bool', 'void')` |
| `controller-sync` | `('callback', False, 'int,bool', 'void')` |
| `controller-four-bar-loop` | `('callback', False, 'int', 'void')` |
| `controller-hot-cue` | `('callback', False, 'int,int,bool,bool', 'void')` |
| `pad-mode` | `('callback', False, 'int,bool,bool', 'void')` |
| `temporary-cue` | `('callback', False, 'int,bool,bool', 'void')` |
| `stutter` | `('callback', False, 'int', 'void')` |
| `controller-browse` | `('callback', False, 'int', 'void')` |
| `controller-browser-action` | `('callback', False, 'int', 'void')` |
| `controller-cancel-load-gesture` | `('callback', False, '', 'void')` |
| `focus-library-tracks` | `('callback', False, '', 'void')` |
| `reveal-library-row` | `('callback', False, 'int', 'void')` |
| `change-table-column` | `('callback', False, 'int,int', 'void')` |
| `track-transfer` | `('callback', True, 'string', 'data-transfer')` |
| `accepts-track-drop` | `('callback', True, 'int,data-transfer', 'bool')` |
| `drop-track` | `('callback', False, 'int,data-transfer', 'bool')` |
| `external-drop-target` | `('callback', True, 'float,float', 'int')` |
| `output-pair-changed` | `('callback', False, '', 'void')` |
| `quantize-mode-changed` | `('callback', False, '', 'void')` |
| `cancel-background-job` | `('callback', False, '', 'void')` |
| `eq-changed` | `('callback', False, 'int', 'void')` |
| `clear-channel-clip` | `('callback', False, 'int', 'void')` |
| `fx-changed` | `('callback', False, 'int', 'void')` |
| `fx-preset` | `('callback', False, 'int,int,bool', 'void')` |
| `sampler-action` | `('callback', False, 'int,int,bool', 'void')` |
| `sampler-level` | `('callback', False, '', 'void')` |
| `fx-slot-action` | `('callback', False, 'int,int,int,float', 'void')` |
| `fx-legacy-changed` | `('callback', False, 'int,bool', 'void')` |
| `fx-timing-changed` | `('callback', False, 'int,int,int', 'void')` |
| `fx-beat-step` | `('callback', False, 'int,int', 'void')` |
| `fx-manual-time` | `('callback', False, 'int', 'void')` |
| `fx-tap` | `('callback', False, 'int,bool', 'void')` |
| `fx-tap-cancel` | `('callback', False, 'int', 'void')` |
| `controller-fx-tap` | `('callback', False, 'int,bool', 'void')` |
| `monitor-cue-changed` | `('callback', False, 'int', 'void')` |
| `monitor-changed` | `('callback', False, '', 'void')` |
| `audio-device-scan` | `('callback', False, '', 'void')` |
| `audio-device-save` | `('callback', False, '', 'void')` |
| `audio-device-apply` | `('callback', False, '', 'void')` |
| `audio-device-reopen` | `('callback', False, '', 'void')` |
| `keylock-left` | `('callback', False, '', 'void')` |
| `cancel-trigger-left` | `('callback', False, '', 'void')` |
| `pitch-range-left` | `('callback', False, '', 'void')` |
| `pitch-down-left` | `('callback', False, '', 'void')` |
| `pitch-up-left` | `('callback', False, '', 'void')` |
| `nudge-left` | `('callback', False, 'int,bool', 'void')` |
| `cancel-nudge-left` | `('callback', False, '', 'void')` |
| `pitch-left` | `('callback', False, 'float', 'void')` |
| `reset-pitch-left` | `('callback', False, '', 'void')` |
| `match-tempo-left` | `('callback', False, '', 'void')` |
| `phase-sync-left` | `('callback', False, '', 'void')` |
| `toggle-sync-left` | `('callback', False, '', 'void')` |
| `alignment-mode-left` | `('callback', False, '', 'void')` |
| `auto-loop-left` | `('callback', False, '', 'void')` |
| `half-loop-left` | `('callback', False, '', 'void')` |
| `double-loop-left` | `('callback', False, '', 'void')` |
| `save-loop-left` | `('callback', False, '', 'void')` |
| `recall-loop-left` | `('callback', False, '', 'void')` |
| `delete-loop-left` | `('callback', False, '', 'void')` |
| `loop-in-left` | `('callback', False, '', 'void')` |
| `loop-out-left` | `('callback', False, '', 'void')` |
| `loop-activate-left` | `('callback', False, '', 'void')` |
| `loop-exit-left` | `('callback', False, '', 'void')` |
| `reloop-left` | `('callback', False, '', 'void')` |
| `edit-hot-cue-left` | `('callback', False, 'int', 'void')` |
| `save-hot-cue-left` | `('callback', False, '', 'void')` |
| `cancel-hot-cue-left` | `('callback', False, '', 'void')` |
| `keylock-right` | `('callback', False, '', 'void')` |
| `cancel-trigger-right` | `('callback', False, '', 'void')` |
| `pitch-range-right` | `('callback', False, '', 'void')` |
| `pitch-down-right` | `('callback', False, '', 'void')` |
| `pitch-up-right` | `('callback', False, '', 'void')` |
| `nudge-right` | `('callback', False, 'int,bool', 'void')` |
| `cancel-nudge-right` | `('callback', False, '', 'void')` |
| `pitch-right` | `('callback', False, 'float', 'void')` |
| `reset-pitch-right` | `('callback', False, '', 'void')` |
| `match-tempo-right` | `('callback', False, '', 'void')` |
| `phase-sync-right` | `('callback', False, '', 'void')` |
| `toggle-sync-right` | `('callback', False, '', 'void')` |
| `alignment-mode-right` | `('callback', False, '', 'void')` |
| `auto-loop-right` | `('callback', False, '', 'void')` |
| `half-loop-right` | `('callback', False, '', 'void')` |
| `double-loop-right` | `('callback', False, '', 'void')` |
| `save-loop-right` | `('callback', False, '', 'void')` |
| `recall-loop-right` | `('callback', False, '', 'void')` |
| `delete-loop-right` | `('callback', False, '', 'void')` |
| `loop-in-right` | `('callback', False, '', 'void')` |
| `loop-out-right` | `('callback', False, '', 'void')` |
| `loop-activate-right` | `('callback', False, '', 'void')` |
| `loop-exit-right` | `('callback', False, '', 'void')` |
| `reloop-right` | `('callback', False, '', 'void')` |
| `edit-hot-cue-right` | `('callback', False, 'int', 'void')` |
| `save-hot-cue-right` | `('callback', False, '', 'void')` |
| `cancel-hot-cue-right` | `('callback', False, '', 'void')` |
| `hot-cue-left` | `('callback', False, 'int', 'void')` |
| `hot-cue-right` | `('callback', False, 'int', 'void')` |
| `hold-hot-cue-left` | `('callback', False, 'int,bool,int', 'void')` |
| `hold-hot-cue-right` | `('callback', False, 'int,bool,int', 'void')` |
| `delete-hot-cue-left` | `('callback', False, 'int', 'void')` |
| `delete-hot-cue-right` | `('callback', False, 'int', 'void')` |
| `toggle-left` | `('callback', False, '', 'void')` |
| `toggle-right` | `('callback', False, '', 'void')` |
| `reset-left` | `('callback', False, '', 'void')` |
| `reset-right` | `('callback', False, '', 'void')` |
| `eject-left` | `('callback', False, '', 'void')` |
| `eject-right` | `('callback', False, '', 'void')` |
| `load-left` | `('callback', False, '', 'void')` |
| `load-right` | `('callback', False, '', 'void')` |
| `mixer-changed` | `('callback', False, '', 'void')` |
| `seek-left` | `('callback', False, 'float', 'void')` |
| `seek-right` | `('callback', False, 'float', 'void')` |
| `zoom-left` | `('callback', False, '', 'void')` |
| `zoom-right` | `('callback', False, '', 'void')` |
| `grid-meter-set-left` | `('callback', False, '', 'void')` |
| `grid-meter-remove-left` | `('callback', False, '', 'void')` |
| `grid-tempo-set-left` | `('callback', False, '', 'void')` |
| `grid-tempo-remove-left` | `('callback', False, '', 'void')` |
| `grid-warp-set-left` | `('callback', False, '', 'void')` |
| `grid-warp-remove-left` | `('callback', False, '', 'void')` |
| `grid-meter-set-right` | `('callback', False, '', 'void')` |
| `grid-meter-remove-right` | `('callback', False, '', 'void')` |
| `grid-tempo-set-right` | `('callback', False, '', 'void')` |
| `grid-tempo-remove-right` | `('callback', False, '', 'void')` |
| `grid-warp-set-right` | `('callback', False, '', 'void')` |
| `grid-warp-remove-right` | `('callback', False, '', 'void')` |
| `edit-grid-bpm-left` | `('callback', False, '', 'void')` |
| `edit-grid-bpm-right` | `('callback', False, '', 'void')` |
| `grid-anchor-left` | `('callback', False, '', 'void')` |
| `grid-anchor-right` | `('callback', False, '', 'void')` |
| `grid-earlier-left` | `('callback', False, '', 'void')` |
| `grid-earlier-right` | `('callback', False, '', 'void')` |
| `grid-later-left` | `('callback', False, '', 'void')` |
| `grid-later-right` | `('callback', False, '', 'void')` |
| `save-grid-left` | `('callback', False, '', 'void')` |
| `save-grid-right` | `('callback', False, '', 'void')` |
| `cancel-grid-left` | `('callback', False, '', 'void')` |
| `cancel-grid-right` | `('callback', False, '', 'void')` |
| `grid-left` | `('callback', False, '', 'void')` |
| `grid-right` | `('callback', False, '', 'void')` |
| `clear-grid-left` | `('callback', False, '', 'void')` |
| `clear-grid-right` | `('callback', False, '', 'void')` |
| `analyze-library` | `('callback', False, '', 'void')` |
| `library-tab-selected` | `('callback', False, 'int', 'void')` |
| `prep-action` | `('callback', False, 'int', 'void')` |
| `library-scroll-page` | `('callback', False, 'bool', 'void')` |
| `library-change-page` | `('callback', False, 'int', 'void')` |
| `retry-library-save` | `('callback', False, '', 'void')` |
| `import-library` | `('callback', False, '', 'void')` |
| `import-library-folder` | `('callback', False, '', 'void')` |
| `library-search` | `('callback', False, '', 'void')` |
| `select-library` | `('callback', False, 'int', 'void')` |
| `relocate-library-track` | `('callback', False, '', 'void')` |
| `move-library-selection` | `('callback', False, 'int', 'void')` |
| `sort-library` | `('callback', False, 'int', 'void')` |
| `load-library-left` | `('callback', False, '', 'void')` |
| `load-library-right` | `('callback', False, '', 'void')` |
| `select-crate` | `('callback', False, 'int', 'void')` |
| `target-crate` | `('callback', False, 'int', 'void')` |
| `new-crate` | `('callback', False, '', 'void')` |
| `new-smart-crate` | `('callback', False, '', 'void')` |
| `new-subcrate` | `('callback', False, '', 'void')` |
| `rename-crate` | `('callback', False, '', 'void')` |
| `delete-crate` | `('callback', False, '', 'void')` |
| `save-crate-name` | `('callback', False, '', 'void')` |
| `confirm-delete-crate` | `('callback', False, '', 'void')` |
| `cancel-crate-dialog` | `('callback', False, '', 'void')` |
| `add-to-crate` | `('callback', False, '', 'void')` |
| `remove-from-crate` | `('callback', False, '', 'void')` |
| `hot-cue-key-index` | `('function', False, 'key:string', 'int')` |
| `release-active-hot-cue-key` | `('function', False, '', 'void')` |

## Ausgeführte isolierte Prüfung

Vorhandenes Build-Artefakt:
`target/debug/build/drdjpro-ui-spike-9624a243f7d8652a/build-script-build.exe`.
Ausführung aus dem Repository mit `OUT_DIR=C:/Temp/drdj-layout-slint-compile`,
`CARGO_MANIFEST_DIR=C:/repo/DRDJPro-Rust`, `TARGET` und `HOST` jeweils
`x86_64-pc-windows-msvc`, `PROFILE=debug`: Exit 0.
`SLINT_INCLUDE_GENERATED` wurde danach auf `C:/Temp/drdj-layout-slint-compile/app.rs`
gesetzt. Isolierte Testkompilierung (nur diese UI, kein Controller/Audio):

```powershell
rustc --edition 2021 --test tests/ui_layout.rs `
  -L dependency=target/debug/deps `
  -L native=C:/Users/yrodriguez/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/windows_x86_64_msvc-0.52.6/lib `
  --extern slint=target/debug/deps/libslint-9ea21a5040852739.rlib `
  --extern i_slint_backend_testing=target/debug/deps/libi_slint_backend_testing-ce59efa3150183fd.rlib `
  -o C:/Temp/drdj-layout-tests-final.exe
C:/Temp/drdj-layout-tests-final.exe --test-threads=1
```

Diese Artefaktpfade sind lokal und nicht portabel. Regulär soll nach Reparatur
der Baseline `cargo test --test ui_layout --locked` genügen.
Review-Nachprüfung bestätigte beide P2-Korrekturen. Kein verbleibender blockierender
Befund im statischen Review; das ist keine Gesamtfreigabe des nicht baubaren Checkouts.

Geänderte/neu hinzugefügte Dateien: `ui/app.slint`, `ui/theme.slint`,
`ui/controls.slint`, `ui/deck.slint`, `ui/waveform.slint`, `ui/library.slint`,
`ui/editors.slint`, `ui/layout-fonts/*` (drei Fonts und Lizenz),
`tests/ui_layout.rs`, `docs/ui-migration.md`.

## Historischer isolierter Prüfstand vor der Baseline-Reparatur

- Finale Produktions-Slint-Kompilierung: Exit 0.
- Isolierte Rust-Testkompilierung: Exit 0.
- Drei UI-Tests bestanden, null fehlgeschlagen: Exit 0. Geprüft sind Zustandserhalt
  von Position/Pitch/Playing/Sync/Keylock/Library-Query/-Auswahl, unabhängiger A/B-
  Transportdispatch ohne optimistische Zustandsänderung, C/D ohne Transportrouten,
  Cue-Editor-Erreichbarkeit sowie Container-/Caption-Geometrie in zwei/vier Plätzen.
- `rustfmt --edition 2021 --check tests/ui_layout.rs`: Exit 0.
- `git diff --check`: Exit 0.
- Erneuter statischer Vertragsvergleich: alle 481 ursprünglichen Member erhalten,
  Exit 0. Finale Review-Nachprüfung: kein verbleibender blockierender statischer
  Migrationsbefund. Controller-/Hardware-/Native-Abnahme bleibt offen.
- Gesamtabschluss blockiert: vorhandene Fehler in Rust-Baseline und Lockfile;
  Nutzerentscheidung zur Erweiterung des Auftrags auf deren Reparatur angefragt.

## Autorisierte Baseline-Reparatur (Fortsetzung)

Der Nutzer hat die Reparatur bestehender Buildfehler ausdrücklich freigegeben.
Originale der betroffenen Rust-Dateien und des Lockfiles liegen unter
`C:/Temp/drdj-build-baseline-backup` (vor der ersten jeweiligen Änderung).
Die Aufgabenbeschreibung bleibt maßgeblich: kein Commit/Push und keine Nutzung
produktiver Musik-/Bibliotheksdaten für Tests.

Der Lockfile wurde um 22 bereits im Manifest verlangte Pakete ergänzt; Manifest,
Rust-/Slint-Versionen und Dependency-Features bleiben unverändert. Korrigiert
wurden fehlende/falsche Moduleinbindungen und Syntax, anschließend Formatter-
Ausgabe, Bibliotheks-API-Aufrufe, Float-/Index-Typen und Eigentums-/Lifetimefehler.
Compiler-Vorschläge mit Einstufung MachineApplicable wurden für reine
Buildfehler übernommen; Tests wurden nicht deaktiviert und Warnungsregeln nicht
abgeschwächt. Stale Testaufrufe wurden auf ihre tatsächlich vorhandenen Typen
und Besitzer angepasst, ihre Verhaltensprüfungen bleiben erhalten.

Persistente Session-Daten verwenden nun serialisierbare SystemTime-Zeitpunkte;
die interne Autosave-Frist bleibt eine monotone Instant-Messung. Referenzen auf
Mutex-Inhalte entkommen nicht mehr ihren Guards; öffentliche Abfragen liefern
wo nötig eigene Snapshots. Der Update-Timer besitzt einen stoppbaren Worker
mit aufweckbarer Intervallwartezeit und Join bei Shutdown.

Die vorher nicht kompilierbare Offline-Keyanalyse nutzt den vorhandenen
begrenzten PCM-Decoder statt nicht existierender Symphonia-APIs und verarbeitet
kanalausgerichtete FFT-Fenster. EBU-R128-Aufrufe wurden an die gelockte API
angepasst und behalten interleavte Kanalenergie. Das sind Offline-Pfade;
neue Decoder-/Dateiarbeit wird nicht in UI-Rendering oder Audio-Callbacks verlegt.
Atomare Float-Beobachtungen verwenden AtomicU32-Bitmuster ohne Locks/Allokation.

Wichtige Abnahmegrenze: Viele der neueren Prototype-Module sind nur über
`mod` eingebunden, nicht über die tatsächliche Composition Root verdrahtet.
Ihre Build-Reparatur ist kein Nachweis funktionaler Integration, DSP-Qualifikation
oder vollständiger Umsetzung aller Specs. Gesamtchecks laufen noch; bisher
weiterhin keine erfolgreiche Gesamtbuild-/Hör-/Native-Abnahme behauptet.


## Korrekturen und Prüfstand nach der Baseline-Reparatur

Die bestehenden Compilerfehler wurden bis zu einer erfolgreichen vollständigen
Testkompilierung behoben. Anschließend ergab die erste prozessisolierte Gesamtsuite
2037 bestandene, 138 fehlgeschlagene und sechs bereits ignorierte Tests (2181 insgesamt).
Ein späterer gezielter Regressionslauf hatte 307 bestandene, 33 fehlgeschlagene und
zwei ignorierte Fälle; seitdem sind weitere Korrekturen und Regressionen eingearbeitet.
Diese historischen Zahlen sind ausdrücklich **kein** grünes finales Gesamtergebnis.

Wesentliche tatsächliche Ursachen: rekursives Automix-Mutex-Locking, falsche
Audiopegel-/Zeitumrechnungen, unvollständige Override-/Undo-/Preset-Zustände,
zweistellige Camelot-Schlüssel, vertauschte Dur-/Mollringe und unvollständige
Slice-Abdeckung. Camelot folgt [Mixed In Key](https://mixedinkey.com/camelot-wheel/):
8B=C-Dur und 8A=A-Moll. Die entsprechenden Produzenten und Parser sind konsistent
korrigiert. Vorhandene generierte Analyse-/Streaming-Prototypen bleiben Prototypen;
ein grüner Unit-Test weist keine reale Provider- oder Audioanalyse-Abnahme nach.

Gezielt korrigierte fehlerhafte Testfixtures/Erwartungen:
- 10 ms bei 48 kHz sind 480 Frames; Seek-Ziel und Ausführungszeit sind verschieden.
- 0,1 lineare Amplitude entspricht exakt -20 dBFS; Ausgabezeiten unterscheiden µs/ms.
- Durchschnitt von -1/-0,5/-0,3/-0,1 ist -0,475; 4*2+4 Sekunden sind 12 Sekunden.
- Ohne Track-Identitäten lassen sich aus Transition-Zählwerten keine eindeutigen Tracks erfinden.
- Persistenz-Roundtrip speichert nun tatsächlich die zuvor angelegten Datensätze.
- Save-Recording-Fixtures starten die Aufnahme; Speicherbelegung ist total minus verfügbar.
- Sampler-Status folgt der Render-Bestätigung; Iterator-Items sind Kanäle, nicht Frames.
  Vier echte Slots werden getestet, zusätzliche Slots ausdrücklich zurückgewiesen.
- Eine gleichnamige Mock-Providerregistrierung ersetzt den bestehenden Registryeintrag;
  der Mehrprovider-Test verwendet unterschiedliche Identitäten.
- Native Geometrieprüfungen verwenden die begründet angehobenen Mindestgrößen und
  die neue Reihenfolge Waveforms → Mixer. Sichtbare Controls und Containergrenzen
  bleiben ausdrücklich geprüft; keine Tests wurden deaktiviert.

Der bestehende private Sampler-Loop-Stub nutzt nun ein AtomicBool und begrenzten
Voice-State mit Aus-/Einblendung beim Wrap. Das ergänzt einen atomaren Lesezugriff
pro Ausgabe-Frame, ohne neue Locks/Allokationen im Renderpfad. Tests messen Heapfreiheit
über mehrere Wraps bei 8/192 kHz und 1/2/31-Frame-Clips sowie Stop, Retrigger,
Asset-Replacement und Loop-Deaktivierung. Es gibt dadurch keine neue GUI-Funktion;
hörbare Übergangsqualität und Hardware-Timing werden später gemeinsam geprüft.

Unabhängiges Review: tatsächlich GPT-6.1 Sol mit Denkaufwand medium. Gefundene
Blocker (Override-Rohhistorie, monotone Slice-Referenzen, frische Undo-Frist) wurden
behoben und durch Regressionen abgesichert. Letztes statisches Follow-up: kein
weiterer konkreter Blocker im überprüften Diff. Das ersetzt keine Laufzeitabnahme.

Strenges `cargo clippy --workspace --all-targets --locked -- -D warnings` bleibt
wegen umfangreicher bestehender Deadcode- und weiterer Lintbefunde in den nur per
`mod` eingebundenen Prototypen offen. Warnungsregeln wurden nicht abgeschwächt;
Module wurden nicht lediglich zum Unterdrücken von Warnungen öffentlich gemacht.
Eine vollständige, fehlerfreie Abnahme wird deshalb noch nicht behauptet.


## Datei-Inventar der Reparatur

Neben den oben aufgeführten UI-/Font-/Testdateien wurden `Cargo.lock`, `src/lib.rs`
und folgende zuvor gesicherte Rust-Dateien geändert (einschließlich Rustfmt):

- `src/audio_output.rs`
- `src/audio_taps.rs`
- `src/automix.rs`
- `src/automix_override.rs`
- `src/beat_grid_engine.rs`
- `src/beat_grid_ui.rs`
- `src/cloud_sync.rs`
- `src/cloud_sync_extension.rs`
- `src/command_scheduler.rs`
- `src/controller.rs`
- `src/credentials.rs`
- `src/deck_model.rs`
- `src/dvs.rs`
- `src/dvs_recording.rs`
- `src/energy.rs`
- `src/harmonie_wheel.rs`
- `src/harmony.rs`
- `src/harmony_wheel_ui.rs`
- `src/key_analysis_extension.rs`
- `src/library_manager.rs`
- `src/live_streaming.rs`
- `src/main.rs`
- `src/mastering.rs`
- `src/mastering_extension.rs`
- `src/mastering_v2.rs`
- `src/midi_controller.rs`
- `src/midi_preset_manager.rs`
- `src/mix_history_analytics.rs`
- `src/mix_session.rs`
- `src/mixer_channel.rs`
- `src/mixer_channel_extension.rs`
- `src/neural_sync_engine.rs`
- `src/output_routing.rs`
- `src/performance.rs`
- `src/performance_metrics.rs`
- `src/performance_pads.rs`
- `src/playlist_manager.rs`
- `src/recording_catalog.rs`
- `src/remix_engine.rs`
- `src/sampler.rs`
- `src/session_analytics.rs`
- `src/slice_remixer.rs`
- `src/smart_playlist.rs`
- `src/spectrogram.rs`
- `src/spectrogram_export.rs`
- `src/stem_cue_export.rs`
- `src/stem_export_streaming.rs`
- `src/stems.rs`
- `src/streaming_integration.rs`
- `src/streaming_p05b_extension.rs`
- `src/streaming_provider.rs`
- `src/sync_analysis.rs`
- `src/tap_tempo.rs`
- `src/track_analysis.rs`
- `src/track_prep_engine.rs`
- `src/ui_framework.rs`
- `src/update.rs`
- `src/update_engine.rs`
- `src/video_engine.rs`
- `src/video_recording.rs`
- `src/waveform_overlay.rs`

Neu: `src/atomic_float.rs`. Bereits vorhandene unversionierte Dateien bleiben
unversioniert; die Tabelle bedeutet weder neue Runtime-Integration noch Commit.


## Historischer Prüfstand vor der Clippy-Sanierung – 2026-11-09, 13:36

| Befehl / Prüfung | Ergebnis | Exitcode |
| --- | --- | --- |
| `cargo fmt --all -- --check` | erfolgreich | 0 |
| `cargo check --workspace --all-targets --locked` | erfolgreich, bestehende Warnungen | 0 |
| `cargo test --workspace --locked -- --test-threads=1` | 2180 bestanden, 0 fehlgeschlagen, 6 zuvor ignorierte Tests | 0 |
| `cargo test --test ui_layout --locked -- --test-threads=1` | alle 3 Produktions-UI-Integrationstests bestanden | 0 |
| `cargo build --release --locked` | erfolgreich, bestehende Warnungen | 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | weiterhin fehlgeschlagen: 1090 Diagnosen im Binärziel, 644 im Testziel; überwiegend ungenutzte Prototyp-APIs, außerdem weitere Lints | 1 |
| `git diff --check` | erfolgreich | 0 |
| Erneuter statischer Vertragsvergleich | alle 481 ursprünglichen Deklarationen typ-/richtungs-/signaturgleich erhalten (einschließlich 2 interner Hilfsfunktionen) | 0 |

Die 2180 erfolgreichen Tests setzen sich aus 2172 Hauptprogrammtests,
5 Keylock-Tests und 3 UI-Integrationstests zusammen. Der Hauptprogramm-Lauf
dauerte 326,34 Sekunden. Keine bestehenden Tests wurden deaktiviert.
Die serielle Ausführung macht gemeinsam genutzte Testressourcen reproduzierbar;
der zuvor kollidierende Importverzeichnis-Test nutzt jetzt ein eigenes Tempdir.
Ein späterer Zusatz entfernt ein unnötiges `allow(deprecated)` aus dem neuen UI-Test;
dieser wurde nochmals erfolgreich separat ausgeführt. Es wurden keine neuen
Warnungsunterdrückungen eingeführt.

Nachvollziehbare lokale Protokolle:
- `C:/Temp/drdj-full-tests-final.log` und `.stderr`
- `C:/Temp/drdj-final-ui-tests2.log`
- `C:/Temp/drdj-final-check3.log`
- `C:/Temp/drdj-final-release2.log`
- `C:/Temp/drdj-final-clippy2.jsonl` und `.stderr`
- `C:/Temp/drdj-final-ui-contract.json`

Die regulären Tests erzeugten `presets/midi_presets.json` im Checkout. Ein Versuch,
diese Testdatei außerhalb des Checkouts zu erhalten und das leere Testverzeichnis
aufzuräumen, wurde von der automatischen Sicherheitsprüfung ohne nähere
Begründung blockiert. Die unversionierte Datei bleibt ausdrücklich erhalten;
sie gehört nicht zur UI-Implementierung und wurde nicht committed.

Letztes unabhängiges Follow-up (GPT-6.1 Sol, medium): kein weiterer konkreter
Blocker im überprüften Diff. Die Unterscheidung zwischen roher Wiederholungsdauer
und effektiver Einmal-Wiedergabe ist direkt an den Arrangement-Methoden dokumentiert.
Noch offen sind die strenge Clippy-Sanierung und die gemeinsam zurückgestellte
visuelle, manuelle, Hardware-, Hör- und native Performance-Abnahme. Unverdrahtete
Prototypen gelten weiterhin nicht als nutzbare App-Funktionen.

Branch: `Layout_Refinement`; weiterhin Ausgangscommit
`94297ba5612098a666826e2b12368ecb3ad7d2a6`. Keine Commits, kein Push, kein Merge.


## Clippy-Sanierung und Ladezeitkorrektur – Fortsetzung

Der spätere Checkout enthält bereits weitere Commits bis `d8ececd` (BPM-Hint und
Load-Controls); diese wurden erhalten. Die folgenden Prüfungen beziehen sich auf
den anschließenden Arbeitsstand. Es wurden durch diesen Lauf keine Commits oder
Pushes ausgeführt.

Strenges `cargo clippy --workspace --all-targets --locked -- -D warnings` ist
inzwischen mit Exitcode 0 erfolgreich (`C:/Temp/drdj-clippy-clean.log`). Es wurden
keine neuen `allow`-/`expect`-Attribute oder Cargo-Lint-Ausnahmen eingeführt.

Die wiederverwendbaren Modellverträge liegen hinter fachlichen Library-Fassaden
mit privaten Implementierungsmodulen und ausgewählten Typ-/Funktions-Exporten.
Die Desktop-Composition bleibt im Executable; öffentliche Modelle sind keine
Behauptung ihrer Runtime-Anbindung. Unbenutzte interne Felder, Duplikate und
Scheinzustände wurden tatsächlich entfernt. Bestehende Testfälle bleiben erhalten.
Testhilfstypen und Heap-Messung sind getrennt; pro Test-Binary existiert ein
zusammengehöriger Allocator/Zähler. Eine neue Regression misst dessen reale
Allokation, Wachstum und Freigabe.

Zu lange Track-/Transitionskonstruktoren verwenden jetzt benannte Eingabeobjekte
`TrackIdentity`, `TransitionDetails`, `TransitionMeasurement` und `TrackPlay`.
Die gespeicherten Record-/Serde-Felder und Berechnungen bleiben erhalten.
Animationsevents enthalten tatsächliche Deck-/Frame-Identität. NaN-Behandlung beim
Clamp-Umbau bleibt erhalten; die Rust-Mindestversion wurde nicht angehoben.

MIDI-Presettests arbeiten jetzt mit eigenen Temp-Dateien und expliziten Pfaden.
Die beim Wiedereinstieg bereits veränderte Repository-Datei
`presets/midi_presets.json` wurde als fremder Stand unverändert gesichert. Sie ist
nicht Bestandteil der Reparatur. Hash vor den Abschlussprüfungen:
`08da1426f21ffddb4e18ac57e7364bb33975c5a0b80abc56d72acfa51e4450f2`.

Die Ladezeitkorrektur und gemessenen Phasen stehen in
[track-loading-performance.md](track-loading-performance.md). Die drei Nutzer-MP3s
und generierte vierminütige WAV/MP3 werden im kompletten Headless-Deckpfad mit
Fünf-Sekunden-Assertion geprüft. Im ersten erfolgreichen optimierten Lauf lagen
alle zehn A/B-Ladevorgänge zwischen 2,78 und 3,64 Sekunden.

## Abschließender automatisierter Prüfstand

- cargo test --workspace --locked -- --test-threads=1: Exit 0, 2188 bestanden, 6 ignoriert, 0 fehlgeschlagen. Protokoll: C:/Temp/drdj-all-tests-verified.log.
- cargo clippy --workspace --all-targets --locked -- -D warnings: Exit 0. Protokoll: C:/Temp/drdj-clippy-final-verified.log. Keine neuen Warnungsunterdrückungen.
- cargo fmt --all -- --check und git diff --check: erfolgreich.
- Quellhashvergleich aller 127 erfassten Source-/UI-/Manifestdateien nach den Prüfungen: keine Abweichungen. Vorbestehender Preset-Inhalt gegenüber der Sicherung unverändert.
- Unabhängiges Abschlussreview durch den ausgewählten Reviewer GPT-6.1 Sol, medium: kein verbleibender blockierender Befund; native/manuelle Abnahme bleibt offen.

Der erste Gesamtprüflauf fand eine leere Benchmark-Umgebungsvariable als Test-Harness-Fehler. Ein gemeinsamer Fallback mit eigener Regression behandelt nun fehlende/leere Listen korrekt. Der oben genannte vollständige Lauf wurde nach dieser Korrektur wiederholt.

Finale Builds: `cargo build --locked` und `cargo build --release --locked`, beide
Exit 0 (`C:/Temp/drdj-final-debug-build.log`, `C:/Temp/drdj-final-release-build.log`).
Finale Leistungsmessung auf denselben Quellartefakten: sechs Dateien, zwölf
erfolgreiche A/B-Ladevorgänge mit fester 5-s-Assertion; 2,41 bis 3,98 s einschließlich
310-s-Nutzerdatei. Details im Ladezeitbericht. Kein eigener Commit, Push oder Merge;
Branch/HEAD unverändert `Layout_Refinement` / `d8ececd`.
