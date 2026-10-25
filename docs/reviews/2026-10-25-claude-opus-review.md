# Einmaliges Claude-Opus-Review und lokale Einordnung

Datum: 2026-10-25. Geprüfter Code-Commit: `896413edd067c75b03eef6949d0c231ae373d84f`.

Das ausdrücklich angeforderte externe Review ist abgeschlossen. Anwendungscode wurde für diesen Auftrag nicht verändert. Die sieben Opus-Befunde unten sind Vorschläge eines statischen Reviews; die folgende Einordnung unterscheidet bestätigte Codepfade, Messungen und noch offene Annahmen. Es ist keine Produktfreigabe.

## Herkunft und Umfang

- Angefordertes Modell und Modellkennung in der API-Antwort: `claude-opus-5-5*`. Das ist die Auskunft des benutzerkonfigurierten Endpoints; eine unabhängige Bestätigung seines internen Routings liegt nicht vor.
- Vollständiger lesbarer Text mit `stop_reason=end_turn`; 224,5 Sekunden für den erfolgreichen Aufruf.
- Zwei API-Versuche: Der erste endete bei `max_tokens=10000` ohne lesbaren Review-Text. Der zweite mit Reserve `24000` lieferte den Bericht. Obwohl für den zweiten `thinking=disabled` angefordert war, enthielt die Antwort Thinking- und Text-Blöcke. Nur der lesbare Review-Text wurde gespeichert.
- 23 Quell-/Konfigurationsdateien: Produktionsabschnitte aller getrackten `src/*`-/`ui/*`-Dateien sowie `Cargo.toml` und `build.rs`, mit ursprünglichen Zeilennummern. Zusätzlich die drei Controller-Nudge-Tests und begrenzte Auszüge aus MIGRATION_PLAN, GUI_COMPONENTS und FEATURE_CATALOG. Die meisten Testmodule, Dependency-Quellen und das vollständige PDF-Handbuch wurden dem Modell nicht übergeben.
- Keine Ausführungswerkzeuge für Opus. Tests unten wurden lokal durch Codex ausgeführt, nicht von Opus. Weder visuelle/native Interaktion noch Hören/physische Latenz wurden geprüft.
- API-Key und Endpoint-Adresse stehen weder in diesem Bericht noch in der Begleitmetadatei. Die Zugangsdaten wurden nicht in den Review-Prompt aufgenommen.
- API-Protokollgrundlage: [Anthropic Messages API](https://platform.claude.com/docs/en/api/messages/create). Die konkrete Modellverfügbarkeit wird vom konfigurierten Endpoint bestimmt.

## Lokale Einordnung durch Codex

Die Prioritäten in der zweiten Spalte stammen von Opus. Sie sind keine unabhängig abgenommenen Fehlerprioritäten.

| Nr. | Opus | Gegenprüfung | Folgerung |
| --- | --- | --- | --- |
| 1 | P1 | **Codepfad bestätigt.** `can_change_crates` (controller.rs:267) sperrt bei jedem laufenden Load/Import; Pitch, Nudge, Loop und Hot-Cues verwenden dieses globale Gate. Beide CompactDecks erhalten globales Loading. Play/Pause, Cue-to-zero und Waveform-Seek bleiben dabei verfügbar. | Bestätigte Einschränkung für gleichzeitiges Laden/Beatmatching. Deck-Busy von Bibliotheks-/Persistenzschutz trennen und Mehrdeck-Regressionen ergänzen. Nicht pauschal sämtliche Schutzbedingungen entfernen. Das bisher explizit getestete Sicherheitsverhalten ist kein neu nachgewiesener Nudge-Verdrahtungsfehler. |
| 2 | P1 | **Verzögerte Positionspublikation bestätigt und offline gemessen.** Rodio0.21.1 `sink.rs:129-141` publiziert im5ms-Takt; `snapshot` (playback.rs:678) liest diesen Wert, `set_rate_state` (389-402) verwendet ihn für den Neustart. Siehe Messung unten. | Quellenersatz verwendet einen gröberen Playhead. Hörbares Stottern, Netto-Richtung kurzer Nudge-Taps und kumulierte Drift wurden nicht gemessen. Vor einer Korrektur wiederholte Flanken samt Queue-/Resampler-Verhalten offline reproduzieren; ein atomarer Decoder-Cursor allein garantiert keine Ausgabetakt-Position. |
| 3 | P2 | **Lebensdauer-Risiko aus Quellen nachvollziehbar, Thread-Messung offen.** Sink::drop setzt nur Stop/Keepalive; `SourcesQueueOutput::next/go_next` und `MixerSource::sum_current_sources` verwerfen Quellen beim Pull. Beim Ersatz fällt die alte Control-Plane-Trackreferenz weg; die letzte PCM-Arc kann im Pull-Pfad liegen. | Freigabe großer Puffer außerhalb des Audio-Pfads sichern und mit Drop-/Thread-Tracker beweisen. Weder Dropout noch eine konkrete Freigabedauer wurde gemessen. Ein weiterer GUI-Tick oder `sink.empty()` allein ist kein ausreichender Retirement-Nachweis. |
| 4 | P2 | **Sprung auf In bestätigt; Bug-Einstufung offen.** `activate_loop` (playback.rs:550-564) startet explizit am Loop-In. Der bestehende Plan und die Tests beschreiben getrenntes Setzen und Aktivieren. Opus hatte das vollständige Referenzhandbuch nicht vorliegen. | Zunächst LOOP/Reloop/OUT-Semantik mit der maßgeblichen Referenz festlegen. Nicht allein aufgrund dieser Modellantwort das bisher bewusst getestete Verhalten ändern. Nahtloses Aktivieren innerhalb des Bereichs ist eine sinnvolle gesonderte Produktentscheidung. |
| 5 | P2 | **O(N)-Arbeit bestätigt; Laufzeitfolgen offen.** `Library::catalog` klont den Katalog auf dem GUI-Pfad; Store::save prüft/sichert den vollständigen Bestand, `write_tracks` liest ihn erneut, upsertet Metadaten und schreibt abhängige Tabellen vollständig. Unveränderte Track-Identitätszeilen werden dagegen nicht vollständig neu eingefügt. | Delta-Persistenz und Snapshot-Kosten priorisieren, aber vorher repräsentative Kataloggrößen messen. Ein33ms-Hitch bei100000 Tracks ist hier nicht gemessen. Backups/Integritätsprüfungen nicht ohne Ersatz für die bestehenden Recovery-/Durability-Garantien entfernen. |
| 6 | P3 | **Fehlerpfad statisch bestätigt.** `Store::open` propagiert einen Fehler des ergänzenden `store.save(&enriched)` (library_store.rs:745-751). Der Controller übernimmt bei `Loaded(Err)` die vorhandenen DB-Tracks nicht und meldet Session-only. Keine erneute Fehler-Injektion für diesen konkreten Pfad durchgeführt. | Erfolgreiches Lesen vom optionalen Backfill-Speichern trennen; bei Schreibfehlern lesbaren Katalog behalten und Retry ermöglichen. Die intakte DB wird durch diesen Fehlerpfad nicht als gelöscht behauptet. |
| 7 | P3 | **Dynamisches Vec-Wachstum und fehlendes Worker-Cancel bestätigt; Peakzahl offen.** PCM wird mit push gesammelt, anschließend in Arc-Speicher überführt. Die256MiB-Grenze begrenzt Sample-Nutzdaten, nicht die gesamte Prozess-Spitze. | Speicher-/Abbruch-Budgets messen. Opus' konkrete512MiB-Schätzung ist nicht gemessen und folgt nicht allein aus Vec-Verdopplung; Vec→Arc, Analysepuffer, zwei Decks und Decoder gehören in die Rechnung. Reservierung nur mit validierter Obergrenze. |

## Zusätzliche Offline-Messung zu Befund2

Ein außerhalb des Repositorys gebautes Rust-Scratchprogramm verwendete unveränderte Produktionsmodule und die bereits gebaute Rodio0.21.1-Dependency. Eine mono48kHz/16bit-Rampen-WAV enthielt als Samplewert den Frameindex. Bei Deck-A-Gain1 und Crossfader0 ließ sich aus dem ausgegebenen PCM der letzte gerenderte Quellframe ablesen. Es wurde an vier Pull-Grenzen mit `snapshot.position*48000` verglichen:

| Letzter ausgegebener Rampenframe | Veröffentlichter Positionsframe | Differenz in Frames |
| --- | --- | --- |
| 4999 | 4800 | 199 |
| 5016 | 4800 | 216 |
| 5047 | 5040 | 7 |
| 5126 | 5040 | 86 |

Die größte dieser vier beobachteten Differenzen beträgt216/48000=4,5ms. Das misst die Positionspublikation, nicht einen vollständigen Quellenwechsel, Geräte-/Resampler-Latenz oder Hörbarkeit. Keine Aussage über eine allgemeine maximale Latenz oder alle Formate. Der erste Scratch-Linkversuch fehlte wegen des nicht angegebenen Windows-Importlib-Pfads; mit dem vorhandenen Windows0.52-Importlib-Verzeichnis wurde das Programm gebaut und lief mit Exit0. Kein Anwendungscode wurde dafür bearbeitet.

## Empfohlene nächste Arbeitspakete

1. Die Mehrdeck-Ladesperre mit einem blockierten Picker reproduzierbar testen und Performance-Steuerung deckweise entkoppeln; Bibliotheks-Restore-/Mutationskonflikte weiter absichern.
2. Quellenwechsel-/Nudge-Flanken mit kurzen Offline-Pulls reproduzieren; danach Ausgabe-Playhead und Source-Retirement gemeinsam entwerfen. Kein pauschales Versprechen, eine SPSC-Queue allein sei bereits echtzeitgeeignet.
3. Backfill-Schreibfehler gezielt injizieren, lesbaren Katalog erhalten und Retry-Verhalten testen.
4. Große Kataloge und Decode-Peaks vermessen, anschließend Delta-Persistenz/Abbruch umsetzen. Vor Änderungen an Backups die vorhandenen Recovery-/Rollback-Tests erhalten.
5. LOOP-/Reloop-Semantik am Handbuch festlegen. Tempo-Matching, Keylock und DSP bleiben geplante Funktionsarbeit; eine noch offene DSP-Auswahl ist keine nachgewiesene zwingende Voraussetzung für gekoppeltes Tempo-Matching.

Korrektur zur Lückenliste der Originalantwort: Elapsed/Total ist bereits implementiert (ui/app.slint:206), ebenso das optionale Auto-Loop-BEAT-SNAP (251). Offen bleiben die Remaining-Time-Anzeige und die allgemeine Trigger-Quantisierung. Auto-Loop-Beat-Snapping wird nicht als globale Quantisierung ausgegeben.

Weitere Aussagen der Originalantwort, etwa zu exakter Gerätelatenz, hörbarer Drift, vollständiger Resampling-Qualität und Power-Loss-Qualifikation, sind keine neuen gemessenen Befunde. Die Aussage zu einer generell fehlenden Equal-Power-Kurve betrifft eine gewünschte Mixer-Erweiterung: aktuell ist die lineare Kurve ausdrücklich definiert. Die Modularisierung des Controllers ist ein Wartbarkeitsvorschlag; sie rechtfertigt keine pauschale Ersetzung aller Properties oder eines funktionierenden Frameworks.

## Lokal erneut ausgeführte Prüfungen

| Befehl | Ergebnis für unveränderten Code896413e |
| --- | --- |
| `cargo fmt --check` | Exit0 |
| `cargo clippy --all-targets -- -D warnings` | Exit0 |
| `cargo test --quiet` |121 bestanden,0 fehlgeschlagen/ignoriert;14,90s; Exit0 |
| `cargo build --release` | Exit0;0,79s mit vorhandenem Build-Cache |

Der erste zusätzliche GPT-6-Astra/Mittel-Gegenprüfungsversuch brach wegen fehlender Workspace-Credits ab und zählt nicht als Review. Ein erneuter Versuch mit demselben ausgewählten Modell und Reasoning medium wurde anschließend abgeschlossen. Astra prüfte die Einordnung, die Originaltext-Erhaltung, lokale Rodio-/UI-/Persistenzquellen und den Ausschluss von Zugangsdaten. Es empfahl die Veröffentlichung nach drei Dokumentationskorrekturen: abgeschlossenen Retry festhalten, vorhandenes Elapsed/Total und Auto-Loop-Beat-Snap ausdrücklich klarstellen sowie die Hash-Normalisierung benennen. Diese Korrekturen sind hier umgesetzt. Astra führte weder externe API-Aufrufe noch die Cargo-Gates oder das Rampenprogramm erneut aus; letztere bleiben Autor-Nachweise. Die reguläre Standing-Review-Vorgabe bleibt unverändert.

Die Begleitmetadatei enthält `raw_review_sha256`: SHA-256 über den aus der API-Antwort gespeicherten lesbaren Review-Text, eingelesen mit universeller Zeilenenden-Normalisierung und als LF-normalisierter UTF-8-Text kodiert. Es ist kein Hash der gegebenenfalls CRLF-kodierten TEMP-Dateibytes.

## Originalantwort von Opus

Die folgende Antwort wird unverändert als Modellbefund wiedergegeben. Bei abweichender Einordnung gelten die oben dokumentierten Einschränkungen; daraus entsteht kein bestätigter Fehlernachweis und keine Freigabe.

---

# Statisches Code-Review – DrDJ Pro, Commit `896413e`

**Rahmen:** Rein statische Durchsicht der gelieferten Quellen. Es wurde nichts ausgeführt. Die vom Autor gemeldeten 121 Tests, fmt, Clippy und Release-Build sind unabhängig davon nicht bestätigt. Aussagen zum Verhalten von Rodio, Slint und SQLite beruhen auf meinem Kenntnisstand zu den gepinnten Versionen und sind als **Annahme** markiert. Fehlende Testmodule werden nicht als fehlende Abdeckung gewertet.

---

## Befunde

### 1. P1 – Globales Busy-Gate koppelt beide Decks und blockiert Performance-Steuerung
**Konfidenz:** hoch, direkt im Code ablesbar.

**Fundstellen:**
- `src/controller.rs:267-271`, `568`, `619`, `638`, `789`, `819`, `986`
- `ui/app.slint:667`, `692`

```rust
fn can_change_crates(&self) -> bool {
    self.storage_ready.get()
        && self.pending.borrow().is_none()        // irgendein Deck lädt
        && self.pending_import.borrow().is_none() // Bibliotheksimport
}
```

Dieses Gate prüfen Pitch, Nudge, Loops, Hot Cues und `apply_pending_pitch`. `cancel_stale_nudges` bricht einen laufenden Nudge sogar ab, sobald das Gate schließt. In Slint gilt `loading: root.loading || root.library-busy` für beide Decks.

**Auslöser:**
- Deck A spielt, Nutzer klickt „LOAD“ auf Deck B.
- `pending` ist bereits gesetzt, solange der native Dateidialog offen ist (`start_load`, Zeile 1481). Der Dialog läuft im Worker.
- Danach folgt der Decode, der bis zu 256 MiB umfassen kann.
- Ebenso: Import von bis zu 5000 Dateien oder der Library-Restore beim Start.

**Auswirkung:**
- Während dieser ganzen Zeit sind auf dem spielenden Deck Pitch, Nudge, Loop und Cue gesperrt.
- Ein gehaltener Nudge wird abgebrochen.
- Ein bereits eingereihter Pitch-Wert wird verworfen („Pending pitch discarded during loading“).
- Damit ist der zentrale DJ-Workflow blockiert: nächsten Track laden, während man beatmatcht.

**Minimaler Fix:**
- Gate aufteilen:
  - `deck_busy(id)`: nur `pending == Some((id, _))`.
  - `library_mutation_allowed()`: Storage-Bereitschaft.
- Performance-Befehle prüfen nur `deck_busy(id)`.
- Cue- und Loop-Persistenz schreibt weiterhin in die `Library`. Das ist unkritisch, denn Import und Load mergen später per `insert`.
- In Slint ein deckspezifisches `loading` verwenden.

**Regression:** Picker per Channel blockieren, sodass Deck B „lädt“, während A spielt. Dann für A assertieren:
- Pitch wird angewendet.
- Nudge bleibt aktiv.
- Hot Cue und Loop funktionieren.

Danach B fertigladen und sicherstellen, dass A unverändert weiterläuft.

---

### 2. P1 – Quellenwechsel an verzögerter `get_pos` wiederholt Audio bei jeder Nudge-, Pitch- oder Loop-Änderung
**Konfidenz:** mittel-hoch. **Annahme:** In Rodio 0.21 aktualisiert `Sink::get_pos` die Position nur im `periodic_access`-Takt von etwa 5 ms.

**Fundstellen:** `src/playback.rs:389-402` und `678-682`, analog `pause` (Zeile 607) und `exit_loop` (Zeile 571).

```rust
let position = self.snapshot(id).position;   // offset + sink.get_pos()*factor
let mut source = track.samples.clone();
source.try_seek(position)...;
self.replace_source(id, track, source, position, playing, grid);
```

**Auslöser:**
- Jede Nudge-Flanke (Drücken und Loslassen).
- Jeder angewendete Pitch-Wert. Beim Fader-Ride sind das bis zu etwa 30 pro Sekunde und Deck.
- Resize bei aktivem Loop, Exit und Pause bei aktivem Loop oder Nudge.

**Auswirkung:**
- Die neue Quelle startet an einer Position, die bis zu etwa 5 ms vor dem bereits konsumierten Sample liegt. Es entsteht ein hörbarer Rücksprung oder Stotterer.
- Der Fehler hat ein Vorzeichen, er läuft also immer rückwärts. Er addiert sich zu einer Phasendrift.
- Beim Nudge ist das gravierend: Ein 100-ms-Tap mit 2 % verschiebt die Phase um nur 2 ms. Der Artefakt kann bis zu 5 ms rückwärts betragen. Kurze Nudge-Taps erzeugen damit netto eine Verschiebung in die falsche Richtung.
- Dazu kommt der harte Schnitt ohne Überblendung. Er ist als bekannte Grenze dokumentiert.

**Minimaler Fix:**
- In `PcmSource` einen `Arc<AtomicUsize>` für den Cursor führen. `next()` schreibt ihn mit Relaxed-Store, etwa pro Frame oder alle N Samples.
- Bei Ersetzung diesen tatsächlich konsumierten Frame verwenden statt `get_pos`.
- Mittelfristig siehe Verbesserung A: Rate in-place ändern statt Sink tauschen.

**Regression:** Offline-Mixer und eine WAV-Rampe, bei der der Sample-Wert dem Index entspricht.
- Zwischen 50 Nudge- bzw. Pitch-Flanken jeweils weniger als 5 ms Samples ziehen.
- Assertieren, dass der Ausgangsindex monoton steigt und es keine Wiederholungen gibt.

---

### 3. P2 – Freigabe großer PCM-Puffer vermutlich im Audio-Callback
**Konfidenz:** mittel. **Annahme:** Rodio dropt entfernte bzw. gestoppte Mixer-Quellen im Audio-Thread.

**Fundstelle:** `src/playback.rs:285-296` (in `install` → `replace_source`)

```rust
*self.decks[id.index()].borrow_mut() = Deck { sink, track: Some(track), ... };
// alter Deck (inkl. PreparedTrack-Arc) wird hier auf dem UI-Thread gedroppt,
// die letzte Arc<[f32]>-Referenz lebt aber im Queue der alten Sink weiter
```

**Auslöser:** Ein neuer Track ersetzt einen alten auf einem Deck, während das andere Deck spielt.

**Auswirkung:**
- Die letzte Referenz auf bis zu 256 MiB PCM wird im Audio-Thread freigegeben, inklusive `munmap` bzw. `free`.
- Das ist ein Dropout-Risiko auf dem live spielenden Deck.
- Die Doku zu R-LOOP-03 nennt Deallokation ausdrücklich als nicht qualifiziert.

**Minimaler Fix:**
- Den alten `PreparedTrack` in einer Control-Plane-„Graveyard“-Liste halten.
- Erst freigeben, wenn die alte Sink nachweislich leer bzw. gestoppt ist, zum Beispiel beim nächsten `tick` nach Bestätigung.

**Regression:** Den Datenpuffer in einen Drop-Tracker einwickeln, der die Thread-ID protokolliert. Nach `install` und anschließendem Mixer-Pull auf einem separaten Thread assertieren, dass der Drop auf dem Control-Thread stattfindet.

---

### 4. P2 – LOOP springt immer auf In und bricht die Phase
**Konfidenz:** hoch für den Code, die Produktsemantik ist zu bestätigen.

**Fundstelle:** `src/playback.rs:560-564`, analog Saved-Loop-Recall mit anschließendem LOOP.

```rust
source.set_loop(region)?;
let offset = frame_time(region.start, source.sample_rate());
self.replace_source(id, track, source, offset, playing, grid);
```

**Auslöser:** Typischer Ablauf:
1. IN drücken.
2. Weiterspielen.
3. OUT drücken. Das aktiviert nichts.
4. LOOP irgendwann später drücken.

**Auswirkung:**
- Sprung von einer beliebigen Position auf In, auch wenn der Playhead innerhalb von [In, Out) liegt.
- Der Takt springt hörbar. Ein nahtloses Einrasten ist nicht möglich.
- In Referenz-DJ-Software aktiviert OUT den Loop nahtlos.

**Minimaler Fix:**
- Liegt der aktuelle Frame in [start, end), Phase beibehalten: `region.wrap(current)`, wie in `apply_loop_region`.
- Nur sonst auf In springen (Reloop).
- Optional: OUT aktiviert den Loop sofort.

**Regression:** In = 1 s, Out = 3 s, Playhead bei 2 s, dann `activate_loop`. Assertieren:
- Die Position bleibt bei etwa 2 s.
- Es gibt keinen Sample-Sprung, geprüft mit dem Rampen-Signal.

---

### 5. P2 – Jede Einzeländerung löst Voll-Snapshot, Voll-Backup und Voll-Rewrite aus
**Konfidenz:** hoch für den Code, der Laufzeiteffekt ist unbelegt.

**Fundstellen:**
- `src/controller.rs:1411` (Snapshot auf dem UI-Thread)
- `src/library_store.rs:761-769`, `481`, `121`, `306-310`, `380`, `410`, `440`, `517-530`

```rust
match worker.save(self.library.borrow().catalog()) {      // UI-Thread: Klon aller Tracks
...
inspect(&self.connection)?;                               // inkl. PRAGMA quick_check
backup_to(&self.connection, &side_path(&self.path, ".bak"))?;
write_tracks(&transaction, catalog)?;                     // ruft erneut inspect()
```

**Auslöser:** Ein einzelner Hot-Cue-Tap bei bis zu 100 000 Tracks.

**Auswirkung:**

| Thread | Aufwand pro Änderung |
|---|---|
| UI-Thread | Klon aller Pfade, Metadaten-Strings, Cues und Loops; ein Frame-Hitch im 33-ms-Tick ist wahrscheinlich |
| Worker | Zweimal `quick_check`, zweimal vollständiges Katalog-Lesen, komplettes Online-Backup mit fsync |
| Worker | Löschen und Neuschreiben aller Crates, Grids, Cues und Loops; Upsert aller Metadatenzeilen mit `synchronous=EXTRA` |

- Das Coalescing über `saving` verhindert Stau, verlängert aber das Fenster mit ungespeicherten Änderungen.

**Minimaler Fix:**
- Dirty-Set pro Track-ID führen und nur betroffene Zeilen per Delta-Upsert schreiben.
- `quick_check` nur beim Öffnen ausführen.
- `.bak` periodisch oder beim Schließen erzeugen statt pro Save.
- Snapshot-Kosten über Arc-geteilte Track-Records senken.

**Regression:** Katalog mit 100 000 Tracks.
- Einen Cue setzen.
- Über `total_changes()` die Zahl geänderter Zeilen prüfen; Ziel ist O(1).
- Zeitbudget-Assertion für `catalog()` bzw. den Snapshot.

---

### 6. P3 – Fehlgeschlagener Metadaten-Backfill macht die Bibliothek für die Sitzung leer
**Konfidenz:** hoch.

**Fundstelle:** `src/library_store.rs:745-751`

```rust
if enriched.tracks.iter().any(|t| pending.contains(&t.id) && t.metadata.scanned) {
    store.save(&enriched)?;   // Fehler => Store::open schlägt komplett fehl
}
```

**Auslöser:**
- Eine bestehende Datenbank mit ungescannten Tracks, etwa nach einer Migration.
- Der Backfill-Save schlägt fehl, zum Beispiel durch volle Platte, Busy-Timeout von 200 ms oder einen Fehler bei Backup oder Rename.

**Auswirkung:**
- Das Event `Loaded(Err)` setzt den Modus „session only“.
- Die Bibliothek erscheint leer, obwohl die Datenbank intakt ist.
- Es entsteht kein Datenverlust, aber eine verwirrende Recovery-Situation.

**Minimaler Fix:**
- Backfill-Fehler als nicht fatal behandeln.
- Bibliothek laden, als `dirty` markieren und den Fehler über den bestehenden Retry-Pfad melden.

**Regression:** Schreibfehler beim Backfill injizieren, etwa durch eine externe Write-Transaktion. Assertieren:
- Die Tracks sind sichtbar.
- „RETRY SAVE“ wird angeboten.

---

### 7. P3 – Decode-Speicherspitze und nicht abbrechbare Worker
**Konfidenz:** mittel.

**Fundstelle:** `src/playback.rs:88-97`

```rust
let mut samples = Vec::new();
for sample in decoder { ... samples.push(sample); }
```

**Auslöser:** Lange Tracks nahe dem Limit von 256 MiB.

**Auswirkung:**
- Durch Verdopplungs-Reallokation entsteht transient etwa der doppelte Speicherbedarf, also rund 512 MiB.
- Dazu kommen zwei bereits geladene Decks.
- Load und Import haben keinen Cancel. In Verbindung mit Befund 1 verlängert das die Sperre.

**Minimaler Fix:**
- `reserve` anhand von `n_frames` bzw. `total_duration` mit Obergrenze.
- Alternativ chunked Storage.
- Ein Cancel-Flag für die Worker.

**Regression:** Peak-Allokation über einen Test-Allocator für eine WAV mit bekannter Länge messen.

---

## Bekannte, geplante Lücken (keine Bugs)

- Sync, Phasen-Sync, Keylock und Quantize fehlen.
- Mixer-DSP fehlt: EQ, Filter, Limiter, Equal-Power-Crossfade, Meter.
- Kein Kopfhörer- oder Cue-Bus, keine Gerätewahl, kein MIDI/HID.
- Kein Click-Schutz beim harten Quellwechsel. Die Positionsanzeige kompensiert keine Gerätelatenz. Loop-In, Auto-Loop und Cue-Set erfassen deshalb um die Ausgabelatenz zu früh.
- Rodio-Resampling ist linear und nicht auf Anti-Aliasing qualifiziert.
- Eject, Elapsed/Remaining-Anzeige, Repeat/Autoplay, Sub- und Smart-Crates, Relocation fehlen.
- Variable Grids und Warp-Marker fehlen.
- Nicht qualifiziert sind native Darstellung, Hörtests, Latenz sowie Power-Loss- und Plattformtests unter Windows/macOS.

---

## Architektur- und Produktverbesserungen (priorisiert)

**A. Echtzeit-Deck-Graph statt Sink-Austausch**
- Pro Deck eine langlebige Quelle.
- Befehle wie Rate, Seek, Loop und Nudge laufen über eine lock-freie SPSC-Queue.
- Die Playhead-Position publiziert der Audio-Thread atomar und framegenau.
- Kurze Crossfades bei Sprüngen.
- Das behebt Befund 2, entschärft Befund 3 und ist Voraussetzung für Sync und Quantize.

**B. Zustandsmodell pro Deck und Job-Queue für die Bibliothek**
- Explizite Zustände je Deck: Idle, Loading, Ready, Playing.
- Library-Jobs (Import, Restore, Save) sind von der Performance-Steuerung entkoppelt.
- Dateidialoge setzen keinen Busy-Status.

**C. Persistenz auf Deltas umstellen**
- Upserts pro Entität.
- Backups periodisch bzw. beim Start und Ende.
- Integritätsprüfung nur beim Öffnen.
- Inkrementeller Metadaten-Scan mit Fortschrittsanzeige.
- Danach Skalierungstests mit 100 000 Tracks.

**D. DSP-Entscheidung vor Tempo-Matching**
- Time-Stretch- und Keylock-Bibliothek sowie Resampler-Qualität festlegen.
- Grund: Manuelles Grid-Tempo-Matching auf gekoppeltem Pitch ist nur ein Zwischenschritt.

**E. Controller und UI entduplizieren**
- `controller.rs` hat rund 2100 Zeilen mit gespiegelten `left-*`/`right-*`-Properties.
- Vorschlag: Deck-Struct bzw. Slint-Global pro Deck und `DeckId`-parametrisierte Callbacks.
- Das reduziert Verdrahtungsfehler und erleichtert spätere Decks 3/4 sowie Controller-Mapping.
