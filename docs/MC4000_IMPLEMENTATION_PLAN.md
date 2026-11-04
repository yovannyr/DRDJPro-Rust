# MC4000: Funktionsluecken und Implementierungsplan

Stand: 2026-11-02, gepruefter Rust-Quellstand `3b8b7e5`.

Fortschreibung: S1 ist implementiert und automatisiert geprueft; native
Regler-/Hoerabnahme bleibt offen. Die Belegstellen unten beschreiben den
urspruenglichen Auditstand, sofern nicht als Fortschreibung gekennzeichnet.

Quelle: [Denon DJ MC4000 Benutzerhandbuch](MC4000-User-Guide-v1.2_00.pdf),
deutsche Seiten 24-27. Die MIDI-Tabelle auf Seiten 30-31 liefert die
technische Zuordnung. Es geht um DrDJ Pro Rust, nicht um eine Integration
von Serato. Der .NET-Vergleich bleibt ausgesetzt.

Dies ist ein Implementierungsplan, keine Behauptung vollstaendiger Paritaet.
Vorhandene Engine-/GUI-Funktionen werden wiederverwendet; fehlende
Controller-Zuordnungen sind keine fehlenden Audio-Algorithmen.
Der Nutzer bestaetigte physisches Jog und Maus-Scratchen als funktionierend.
Eine gemessene Latenz-, Langzeit- oder vollstaendige Hardwareabnahme folgt
daraus nicht.

## 1. Inventar der noch fehlenden Funktionen

**MIDI fehlt** = Funktion in der App vorhanden, Bedienung am MC4000 fehlt.
**Teilweise** = ein Teil funktioniert, Handbuchverhalten ist unvollstaendig.
**App fehlt** = neue Anwendungs-/Domain-/Audiofunktion erforderlich.
Die Nummern beziehen sich auf die Bedienelemente des Handbuchs.

| Seite / Element | Funktion | Status und konkrete Luecke |
| --- | --- | --- |
| 24 / 1-3 | Kanalpegel, Drei-Band-EQ, bipolarer Filter | **S1 implementiert.** CC22-26 steuern vorhandenes Trim/EQ/Filter fuer A/B. Neutral63/64, Endpunkte und coalescierte Regler getestet; native Regler-/Hoerabnahme offen. |
| 24 / 7 | SHIFT | **MIDI fehlt.** Globale SHIFT-Halte-/Freigabesteuerung fehlt. Viele Tasten senden bereits eigene Shift-Noten; Jog braucht den Modifierzustand. |
| 24 / 8 | SYNC / SHIFT: Sync aus | **MIDI fehlt.** Tempo-/Phasen-Sync ist in Engine und GUI vorhanden; Note2/3 fehlen. Sync aus muss explizit deaktivieren, nicht erneut toggeln. |
| 24 / 9 | Temporaerer Transport-CUE | **Teilweise.** Aktuell Cue-to-zero. Temporaeren Punkt bei Pause setzen, gedrueckt vorhoeren, beim Loslassen zurueckkehren/pausieren und mit PLAY uebernehmen fehlen. Gespeicherte Hot-Cues sind davon getrennt. |
| 24 / 9 | SHIFT+CUE: Track Start | **MIDI fehlt.** Vorhandenen Cue-to-zero-Pfad gezielt mit Shift-Note5 verbinden. |
| 25 / 10 | SHIFT+PLAY: Stutter | **App fehlt.** Wiederholtes Starten am zuletzt gesetzten temporaeren Cue; Note4 wird ignoriert. |
| 25 / 11 | Vier Hot-Cue-Pads / SHIFT: loeschen | **MIDI fehlt.** Acht persistente App-Slots und Preview sind vorhanden, physische vier Pads haben bislang nur LED-Feedback. Pad1-4 auf Slots1-4 abbilden; Slots5-8 nicht versteckt erfinden. |
| 25 / 12 | Pad-Modus | **Implementiert S15:** pro Deck Hot Cues/Sampler, GUI MC-Modus und Note127 Kanal11/12, Modus-/Pad-/Sampler-LEDs. Acht GUI-/Keyboard-HotCues bleiben unabhaengig; Roll/Slicer/Flip nicht angeboten. Native Abnahme offen. |
| 25 / 13 | Jog zur Loop-In/Out-Bearbeitung | **S9 implementiert.** AUTO-Panel waehlt Off/In/Out; Hardware In/Out schaltet Edit bei aktivem Loop. 1ms/Einheit, Samplegrenzen und mindestens ein Frame; native Abnahme offen. |
| 25 / 13 | SHIFT+Jog: schnelle Tracknavigation | **S9 implementiert.** 50ms Quellzeit/Einheit, Grenzen und Play/Pause erhalten; Scratch-Besitz und Modifier-Freigabe gesichert. Native Sensitivitaet offen. |
| 25 / 14 | SHIFT+VINYL: Censor | **Implementiert S11:** separater Reverse-Kopf mit weiterlaufender normaler Zeitlinie; Release kehrt auf diese zurueck. Manuelles Scratchen bleibt getrennt. |
| 25 / 15 | SHIFT+KEY LOCK: Pitch-Bereich | **MIDI fehlt.** App-Pitchbereiche vorhanden; Shift-Note14 fehlt. |
| 25 / 16 | Pitch-Bend -/+ | **MIDI fehlt.** Vorhandenen momentanen Nudge an Note12/11 mit Release anbinden. |
| 25 / 16 | SHIFT+Pitch-Bend: Rueck-/Vorspulen | **S9 implementiert.** Note32/33 mit Release, globales SHIFT erforderlich, 8s Quellverschiebung pro Sekunde am Steuerthread; Gap ueber100ms beendet ohne Nachholsprung. Kein kontinuierlicher Reverse-DSP. |
| 25 / 17 | Hochaufloesender Pitch-Fader | **MIDI fehlt.** App-Pitchsteuerung vorhanden; zusammengehoerige CC41 MSB / CC9 LSB noch nicht verarbeitet. Pickup nach GUI-/Bereichswechsel fehlt. |
| 25 / 18-20 | Drei unabhaengige FX und Auswahl | **Implementiert S13:** drei serielle Slots je Deck, Delay/LP/HP, einzelne Aktivierung/Intensitaet und SHIFT-Auswahl. Bisherige Wet/Feedback/Zeit-Belegung bleibt als sichtbarer Legacy-Modus; neue Slot-Belegung wird explizit gewaehlt. |
| 25 / 19,21 | FX-Beats, Tap-Tempo, BPM-Reset | **Implementiert S12:** BEATS waehlt Beat-Unterteilungen; TAP Note4 mit Releases, begrenztem Median und Hold-Reset auf Deck-BPM. Fehlende/zu grosse Zeitbasis fuehrt zu sichtbarem Bypass. |
| 25-26 / 22-25 | Auto-Loop, halbieren, verdoppeln, Reloop, SHIFT In/Out | **MIDI fehlt.** Engine-/GUI-Funktionen vorhanden; Loop-Noten auf Kanaelen5/6 fehlen. |
| 25 / 22 | SHIFT+AUTO: Auto4 | **Teilweise.** Quelle beschreibt vier Takte. Vorhandene Auto-Loops sind beatbasiert, kein ausdruecklicher Vier-Takte-Aufruf. Nicht als vier Beats implementieren; Umgang mit Taktwechseln festlegen. |
| 26 / 26,28 | Browse/Library, schnelle Auswahl, Back/Panel | **MIDI/Navigation fehlt.** Suche, Auswahl, Paging und Tabs vorhanden; Encoder, Druck, Shift-Schnellwahl sowie klarer Fokus-/Panelpfad fehlen. |
| 26 / 27 | LOAD A/B | **MIDI fehlt.** Selektierten Track laden funktioniert in der GUI; globale LOAD-Noten2/3 fehlen. Bestehenden Schutz spielender/beschaeftigter Decks erhalten. |
| 26 / 27 | LOAD-Doppeldruck: Instant Doubles | **S7 implementiert.** Bis350ms am Steuerthread; geteilter Track, Softwareposition/Basistempo/Grid/Keylock/Playzustand. Erster Druck normaler Library-LOAD, zweiter ersetzt nur dessen eigenen Job; laufendes Ziel und Konflikte geschuetzt. Ziel-Loops/TempCue zurueckgesetzt, Mixer erhalten. Details im GUI-Inventar, native Abnahme offen. |
| 26 / 29 | Load Prep / Open Prep | **S8 implementiert.** Eigene persistente Prep Queue mit geordneten Track-IDs; Note27 hinzufuegen, Note16 oeffnen, GUI Entfernen/Up/Down. Prepare-Analyse bleibt getrennt; native Bedienabnahme offen. |
| 26 / 30-31 | Vier Sampler-Pads, SHIFT Stop, Sample-Pegel | **Implementiert S14:** vier One-Shot-/Restart-Slots, GUI Load/Play/Stop/Clear, Note33-36/40-43 und CC26; fester Masterbus, begrenzte PCM-Puffer und SQLite-Bank ohne Autoplay. |
| 26 / 32 | Situationsabhaengige Hardware-Pegelanzeigen | **S10 implementiert.** Fader geschlossen: Pre A/B; sobald einer offen: Stereo-Master auf beiden, ausser laufendem Cue bei geschlossenem eigenen Fader. Frischekontrolle statt GUI-Clip-Hold, native LED-Abnahme offen. |
| 27 / Vorderseite4 | Split Cue | **S10 implementiert.** MON-Checkbox und absolute Note28; Mono-Cue links / Mono-Master rechts auf3/4, Gain/Mix aktiv,5ms Blend und geraetespezifische Profile. Routing-/Formatkonflikte geschuetzt; native Kopfhoererprobe offen. |

### Bereits vorhanden; nicht neu implementieren

- PLAY/PAUSE, normale Vinyl-Umschaltung, Keylock, Touch/Jog/Scratch und
  virtueller Maus-Scratch samt Drag-Hand.
- Kanal-Fader, Crossfader, Kanal-Kopfhoerer-Cue, Monitor Level und Monitor Pan.
- Unabhaengiger MASTER-/Pre-Fader-Cue-Signalweg auf geeigneten Mehrkanalgeraeten.
- MIDI-Verbindung, manuelles Reconnect, Ausgabefehler-Rueckmeldung und
  begrenztes LED-Feedback. Weitere LED-Zustaende sind mit den neuen Funktionen
  zu ergaenzen; das Start-Lauflicht ist keine funktionale Pad-Anbindung.

### Hardwareseitige Funktionen: kein Software-Backlog

| Quelle | Hardwareumfang | Konsequenz |
| --- | --- | --- |
| 26 / 33-35; 27 / Vorderseite5-6 | Mic1/2-Pegel und Peak, Mic EQ, Echo/Pegel, Talkover/Post | Das Handbuch sagt ausdruecklich, dass Mikrofonsignale direkt zu den Ausgaengen und nicht durch die Software gehen. Keine erfundene App-Mikrofonsteuerung. |
| 26 / 36-38 | MASTER, BOOTH, AUX-Pegel | Keine MIDI-Zuordnung in der Herstellerliste. Native Hardware-/Routingprobe, nicht als fehlenden App-Regler deklarieren. |
| 27 / Rueckseite1-12; Vorderseite1 | Netzteil, USB, Ausgaenge, Stereo/Mono-Schalter, Mic/Aux-Buchsen, Kopfhoererbuchsen, Sicherung | Physische Anschluesse/Schalter. App muss ihr vorhandenes Audiogeraet-/Ausgangsrouting korrekt nutzen; keine Simulation dieser Hardware noetig. |

## 2. Belegstellen im aktuellen Rust-Code

| Bereich | Quellbeleg / Aussage |
| --- | --- |
| Aktueller MIDI-Umfang | [mc4000.rs](../src/mc4000.rs#L8), Command/decode; [Dispatch](../src/mc4000.rs#L512): keine Trim/EQ-, Sync-, Loop-, Hot-Cue-, Browse-, Load-, Split-/Sampler- oder Pitch-Fader-Commands. Transport-CUE ruft reset-left/right auf. |
| Trim/EQ/Filter | [Mixer-DSP](../src/mixer_dsp.rs#L15), [Playback-APIs](../src/playback.rs#L1765). Gemeinsamer pre-fader DSP-Pfad; Pegelabgriff danach. |
| Cue/Monitor | [MonitorSettings](../src/output_routing.rs#L12), [Cue-Bus](../src/cue_bus.rs#L1), [Playback-APIs](../src/playback.rs#L1677). Gain/Mix vorhanden, kein Split-Modus. |
| Scratch und Cue-Trennung | [Platter-APIs](../src/playback.rs#L855), [quantisierter Cue](../src/playback.rs#L945), [Transport-CUE](../src/playback.rs#L1567), [persistente Hot-Cues](../src/hot_cues.rs#L46). Quantisierter/gespeicherter Cue ersetzt keinen temporaeren Transport-Cue. |
| Sync/Pitch | [Sync-Control](../src/sync_control.rs#L1), [Pitch-Bereiche](../src/pitch_control.rs#L1); vorhanden, Anbindung fehlt. |
| Loops | [Auto-Loop](../src/playback.rs#L1390), [Controller-Anbindung](../src/controller.rs#L2577), [Taktmetrik](../src/beatgrid.rs#L129). Auto-Loop verwendet Beatunterteilungen, nicht automatisch vier Takte. |
| FX-/Routing-Qualifikation | [Migrationsplan](MIGRATION_PLAN.md): bestehendes Delay, GUI-Parameter/Persistenz und MC4000-Reglerprobe; vorhandenes unabhaengiges Cue plus Gain/Mix/Profil-Neustart. |
| Prepare-Abgrenzung | [GUI-Inventar](GUI_COMPONENTS.md): Analyse-Prepare ist keine eigenstaendige kuratierte Queue. |

## 3. Hersteller-MIDI fuer die geplanten Slices

Kanalnummern hier **1-basiert wie im Handbuch**; Rust verwendet Nibbles0-15.
Tasten: NoteOn mit Wert>0 als Druck, NoteOff oder NoteOn0 als Release.
LED-Ausgabe verwendet laut Seite30 dieselbe Adresse wie der Eingabebefehl.
Hot-Cue/Pad-Mode-Farben sind0-3, nicht beliebige RGB-Werte.

| Gruppe | Kanal / Befehl |
| --- | --- |
| Mixer | Deck1/2: CC22 Trim,23 High,24 Mid,25 Low,26 Filter; CC28 Fader bereits vorhanden |
| Pitch | Deck1/2: CC41 MSB + CC9 LSB; Note13 Keylock bereits vorhanden,14 Bereich |
| Transport | Deck1/2: Note2 Sync,3 Sync aus,1 Cue,5 Trackstart,0 Play,4 Stutter;11/12 Bend+,Bend-,33/32 schnelle Suche |
| Jog/Vinyl | Deck1/2: CC6 Jog,Note6 Touch und7 Vinyl vorhanden;16 Censor |
| SHIFT/Split | Kanal16: Note50 SHIFT-Hold; Note28 Split Cue |
| Loops | Kanal5/6: Note50 Auto,63 Auto4,52 halb,53 doppelt,56 In,57 Out,51 Reloop;62 als Shift-Reloop gelistet, Verhalten nicht auf Seiten24-27 beschrieben |
| Hot-Cue/Pad | Kanal5/6: Note20-23 Pads,28-31 Loeschen; Kanal11/12: Note127 Pad-Modus |
| FX | Kanal9/10: Note0-2 FX1-3,11-13 Auswahl,4 Tap; CC0-2 Pegel,11-13 Shift-Regler,CC3 Beats; Note65 Beats und Shift-Tap10 zusaetzlich gelistet, nicht ohne festgelegte Semantik implementieren |
| Library | Kanal16: CC0 Browse,CC1 schnell; Note31 Encoderdruck,2 LoadA,3 LoadB,7 Back,17 Panel,27 LoadPrep,16 OpenPrep |
| Sampler | Kanal16: Note33-36 Start,40-43 Stop,CC26 Pegel |
| Meter | Kanal16: CC68 links,69 rechts,24 beide; bestehende Segmentgrenzen beibehalten |

Fuer relative Encoder, 14-Bit-Paare, Shift-Jog und nicht beschriebene
Shift-Varianten sind begrenzte native MIDI-Aufzeichnungen vorzusehen.
Keine Sensitivitaet oder zeitliche Garantie aus Tabellenadressen ableiten.

## 4. Implementierungsreihenfolge in begrenzten Slices

Jeder Slice liefert GUI-/Controller-Zustandsabgleich, explizite Fehler,
Regressionen und passenden Dokumentationsstand. Nach Nutzeranweisung vom
2026-11-03 entfaellt der unabhaengige Review fuer die restlichen Slices S8-S15;
automatisierte Tests und finale Checks bleiben vor Commit/Push erforderlich.
Vorhandene Callbacks/Engine-APIs werden genutzt, keine parallele zweite Engine.

| Slice | Umfang / Abhaengigkeit | Abnahme und Tests |
| --- | --- | --- |
| S1 Mixer-Anbindung | **Implementiert:** CC22-26; bestehende Trim/EQ/Filter-APIs, kein neues DSP. | A/B-Isolation, Mitte/Endpunkte, Neutral63/64, Coalescing und Audio-/GUI-Rampen automatisiert geprueft; native jeden Regler mit MASTER/Cue noch gegenpruefen. |
| S2 Modifier und Pitch | **Implementiert:** globaler SHIFT-Zustand (Note50 ch16), 14-Bit-Pitch (CC41/MSB + CC9/LSB gekoalesziert), Bereich (Note14) und Bend (Note11/12) mit Pickup und Freigabe bei Disconnect/Reconnect/Overflow. | MSB/LSB-Reihenfolge und unvollstaendige Paare, Endpunkte/Mitte, Modifier/Dekodierung, A/B-Isolation, Pickup- und Bereichswechsel-Disarm, Lifecycle-Freigabe automatisiert; native Fader-/Bend-Aufzeichnung und Haptik weiter erforderlich. |
| S3 Sync und Loop-Tasten | **Implementiert:** Sync ein/aus, Auto toggle/halb/doppelt/In/Out/Reloop und SHIFT+AUTO vier Takte; Sync-/Loop-LEDs. | Idempotenz, Loopgrenzen/EOF und A/B automatisiert; vier Takte mit Nenner, Takt-/Tempowechsel und Warp geprueft. Native Sync-/Loop-Hoerprobe offen. |
| S4 Hot-Cue-Pads | **Implementiert:** Pads1-4 Set/Recall/Preview, SHIFT Delete, MIDI-Release und Hardware-Recovery; LED auf naechste unterstuetzte Blau/Rot/Violett-Farbe. | Hold/Release/PLAY und GUI-/Hardware-Besitz automatisiert; bestehende Busy-/Quantize-/SQLite-Pfade erhalten. Acht App-Slots erhalten; native Pad-/LED-Abnahme offen. |
| S5 Temporaerer CUE und Stutter | **Implementiert:** separater temporaerer Punkt je Deck/Quelle; CUE Hold/Release, PLAY-Uebernahme, SHIFT+CUE Trackstart und SHIFT+PLAY Stutter. GUI CUE/START/STUTTER nutzen dieselben Pfade. | Set/Preview/Return/Latch, Hardware-/GUI-Besitz, Pad-Konflikt und Sourcewechsel automatisiert; Fenster-/Lifecycle-/EOF-Freigabe. Temporaer, keine Hot-Cue-/SQLite-Aenderung. Native Abnahme offen. |
| S6 Browse und normales LOAD | **Implementiert:** Encoder normal/zehnfach, Tracks/Crates per Druck, Back zu Crates, Panel zyklisch durch App-Librarytabs; LOAD A/B. | Seitenuebergreifende Auswahl/Queue, sichtbare Zielzeile, Editor-/Suchfokus, Revisionwechsel und Busy/Playing/Missing-Fehler automatisiert. Native Bibliotheksnavigation offen. |
| S7 Instant Doubles | **Implementiert:** zwei aufeinanderfolgende LOAD auf dasselbe Ziel bis350ms; erster normaler LOAD sofort, eigener Ladejob beim erfolgreichen Klonen verworfen. Shared PCM/Keylock, Quellposition/Basistempo/Grid/Playzustand; Ziel pausiert erforderlich. | A/B, Grenzzeit, Unterbrechung, Quelle/Zielwechsel, spaete Ladeergebnisse, Keylock-Pitch und Render-Allokationen automatisiert. Ziel-Loops/TempCue zurueckgesetzt, Mixer/EQ/FX erhalten; Konflikte abgewiesen. Vertrag im GUI-Inventar, native Abnahme offen. |
| S8 Prepare-Queue | **Implementiert:** eigener Prep-Queue-Tab, Note27/16, Deduplizierung und kuratierte Reihenfolge bis5000IDs; Remove/Up/Down, SQLite17 und Delta-/Retry-Persistenz. | Reihenfolge/Suche, neue Aenderungen nach Save-Ack, Migration16, Rollback/Retry, Missing/Relocation, Neustart und GUI-/MIDI-Isolation automatisiert. Kein Analyse-/Autoplay-Start; native Bedienabnahme offen. |
| S9 Erweiterte Jog-Navigation | **Implementiert:** SHIFT-Jog50ms/Einheit, gehaltene Suche8s/s, Loop-Jog1ms/Einheit mit explizitem Off/In/Out. | Grenzen/ein Frame, Stereo-PCM, Play/Pause/A-B, Releases/Opposition/100ms-Gap, Quell-/Renderer-/Transportwechsel, GUI-Modus/Geometrie und Maus-Besitz automatisiert. Native Hoer-/Haptik-/Latenzabnahme offen. |
| S10 Split Cue und Meterpolitik | **Implementiert:** Split in MON/Note28, gemittelte Mono-Signale und5ms Blend, Profilfeld mit Defaultaus; kontextuelle Hardwaremeter mit separatem Stereo-Master-Peak und100ms Produzentenbeobachtung. | Mono/Stereo/6-Kanal-Routing, Frame-/Sync-Kohaerenz, Master-Isolation, Allokationsfreiheit/Release-Zeitbudget, Profil-Neustart4/2/4, Konflikte und Meterumschaltung automatisiert. Native Kopfhoerer-/LED-Abnahme offen. |
| S11 Censor | **Implementiert:** Note16 Hold/Release, eigener Reverse-Kopf und normale vorlaufende Zeitlinie; CENSOR-Anzeige, Loop-Wrap und vorbereiteter Keylock. | Exakte Samples/Phase bei mehreren Tempi, Stereo-/Frame-Kohaerenz, Anfang/EOF, Pause/Seek, Sync-/Scratch-/Cue-Konflikte, Keylock-Pitch, Focus/SHIFT/Recovery und Quellwechsel automatisiert; Heap/optimiertes Renderbudget geprueft. Native Uebergaenge/Hoer-/Latenzabnahme offen. |
| S12 Musikalische FX-Zeitbasis | **Implementiert:** Millisekunden/Deck-/Tap-BPM,1/16 bis4 Beats, Note4 TAP mit1s Hold-Reset, persistente Timing-/Preset-Felder; vorhandener Delay-DSP bleibt. | Gridsegment/Pitch, Tap-Ausreisser/Timeout/Ownership, Bypass/Resume, Legacy/SQLite/Latest-Ack/Neustart, GUI-Grenzen und Stereo-Rampen/Heap/Release-Budget automatisiert. Native Hoer-/Timing-Abnahme offen. |
| S13 Drei FX-Slots und Auswahl | **Implementiert:** drei serielle Delay/LP/HP-Slots je Deck, FX1-3/SHIFT-Auswahl und Pegel/Feedback, eigener Legacy-Reglermodus, Kettenpresets und Slot1-Migration. | Impulsreihenfolge, Filter/Cue/History, sechs aktive Slots/Heap/Release-Budget, Format-Speichergrenze, MIDI/GUI-Isolation und Preset-/SQLite-/Device-Neustart automatisiert. Native Uebergaenge/Hoer-/Latenzabnahme offen. |
| S14 Vier-Slot-Sampler | **Implementiert:** vier One-Shot-/Restart-Stimmen, vorbereitete Mono-/Stereo-Clips, fester Stereo-Masterbus, globale Pegelregelung und GUI/MIDI; persistente Pfade/Level, kein Autoplay. | Vierfachmix/Headroom, Sampleraten/Frames, Retrigger/Stop/EOF, Cue/Master/Deck-Isolation, Generationen/gleichzeitige Installation, Decodergrenzen, Recovery/SQLite/Neustart/Geraetewechsel und Heap/optimiertes Budget automatisiert; native Hoer-/Timing-Abnahme offen. |
| S15 Pad-Modus und Gesamtqualifikation | **Software implementiert:** pro Deck Hot Cues/Sampler, Note127 Kanal11/12 und GUI-Auswahl, zustandsbasiertes Modus-/Pad-/Sampler-LED-Feedback. Keine weiteren Serato-Modi. | Automatisierte Modus-/Hold-/Ownership-/Fokus-/Recovery-/LED- und Workspace-Regressionen; native Gesamtqualifikation weiterhin **OFFEN**: A/B-Testmatrix, reale Tracks, Latenz/Langzeit und Recovery messen. |

Gemeinsamer nativer [Hardware-Testplan S1-S15](MC4000_HARDWARE_TEST_PLAN.md).
Nach unzuverlaessigen QWEN-/DeepSeek-Implementierungsversuchen gilt der
autorisierte Fallback: S2-S6 selbst fertigstellen. Separat wurde S7 mit vom
Nutzer autorisiertem GPT-6.1 Sol umgesetzt. S8 folgt als eigener Queue-Slice.
S9 ergaenzt Navigation und Loop-Jog, S10 Split Cue/Hardwaremeter. S11 ergaenzt Censor, S12 die musikalische Delay-Zeitbasis, S13 drei FX-Slots und S14 den Vier-Slot-Sampler; S15 ergaenzt die zwei App-Padmodi und LED-Zustaende. Die native Gesamtqualifikation bleibt offen. Der
Testplan kennzeichnet offene Funktionen als nicht testbar.

**Empfohlener Start: S1, danach S2-S4.** Damit werden zuerst viele bereits
vorhandene App-Funktionen am Controller nutzbar, bevor neue Audiofunktionen
wie Censor, FX-Ketten und Sampler entstehen. Kein Slice implementiert
automatisch alle nachfolgenden Slices.

## 5. Architektur, Risiken und Abschlusskriterien

- MIDI-Callback bleibt frei von Engine/UI-Zugriff. Geordnete begrenzte Queue
  fuer Gesten, coalescierte Werte fuer absolute Regler; gepaarte Pitchwerte
  nicht als zwei getrennte Tempospruenge anwenden.
- Jede gehaltene Funktion braucht Release und sichere Freigabe bei
  Overflow, Fokus-/Moduswechsel, Reconnect, Sourcewechsel und Audiofehler.
  Eingabebesitz zwischen Maus und Hardware erhalten.
- UI ist Control Plane, DSP ist Render Plane. Reverse/Censor, Sampler und
  neue FX muessen ohne Locks, Dateizugriff oder Allokationen im Audiocallback
  arbeiten; CPU-/Speicherbudgets und Timing messen.
- Controller/GUI duerfen sich nicht gegenseitig mit veralteten Werten
  ueberschreiben. Bestehende native Gain/Mix-, FX- und Scratch-Ergebnisse
  gelten als Regressionen, nicht als Freibrief fuer neue Semantik.
- Kleine relevante Tests je Slice; bei gekoppelten Audio-/Sync-Aenderungen
  Workspace-Suite, Formatierung, striktes Clippy und Release. DSP braucht
  zusaetzlich Sample-/Timing-/Allokationsnachweise und native Hoerprobe.
- Dokumentations-Haken erst fuer den exakt geprueften Umfang setzen.
  Physische Bedienelemente, Shiftvarianten, LED-Status, Routing und
  Recovery bleiben separat in einer Hardware-Testmatrix nachweisbar.
- App-Funktion, Controller-Anbindung und native Qualifikation getrennt
  bewerten. Keine vollstaendige Produktfreigabe durch Headless-Tests.
