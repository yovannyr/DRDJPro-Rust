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
| 25 / 12 | Pad-Modus | **App/MIDI fehlt.** Kein Moduswechsel fuer die Controller-Pads. Das Handbuch nennt erweiterte Serato-DJ-Modi, aber keine abschliessende Modusliste: Roll/Slicer usw. sind hier keine bereits spezifizierten Anforderungen. |
| 25 / 13 | Jog zur Loop-In/Out-Bearbeitung | **App/MIDI fehlt.** Loop-Grenzen existieren; ein expliziter In-/Out-Editmodus mit Jog-Steuerung fehlt. |
| 25 / 13 | SHIFT+Jog: schnelle Tracknavigation | **MIDI/Verhalten fehlt.** Normales Jog/Scratch funktioniert; Modifier-basierte schnelle Suche fehlt. |
| 25 / 14 | SHIFT+VINYL: Censor | **App fehlt.** Rueckwaertswiedergabe mit parallel weiterlaufender normaler Zeitlinie und Rueckkehr auf deren Position beim Loslassen. Manuelles Scratchen ist kein Censor. |
| 25 / 15 | SHIFT+KEY LOCK: Pitch-Bereich | **MIDI fehlt.** App-Pitchbereiche vorhanden; Shift-Note14 fehlt. |
| 25 / 16 | Pitch-Bend -/+ | **MIDI fehlt.** Vorhandenen momentanen Nudge an Note12/11 mit Release anbinden. |
| 25 / 16 | SHIFT+Pitch-Bend: Rueck-/Vorspulen | **App/MIDI teilweise.** Seek ist vorhanden; gehaltene, begrenzte Schnellnavigation mit Note32/33 fehlt. Geschwindigkeit und Freigabeverhalten sind vor dem Slice festzulegen. |
| 25 / 17 | Hochaufloesender Pitch-Fader | **MIDI fehlt.** App-Pitchsteuerung vorhanden; zusammengehoerige CC41 MSB / CC9 LSB noch nicht verarbeitet. Pickup nach GUI-/Bereichswechsel fehlt. |
| 25 / 18-20 | Drei unabhaengige FX und Auswahl | **Teilweise, derzeit abweichend.** Ein Delay je Deck. Die drei Regler steuern dessen Wet/Feedback/Zeit, nicht drei FX-Pegel. FX2/FX3-Tasten und Shift-Auswahl fehlen. Die historischen Pakete FX1/FX2/FX3 im Migrationsplan sind keine drei Effektslots. |
| 25 / 19,21 | FX-Beats, Tap-Tempo, BPM-Reset | **App/MIDI fehlt.** BEATS aendert aktuell Delayzeit in 10-ms-Schritten. Musikalische Beat-Unterteilungen, Tap-Zeitbasis und Halten-zum-Reset fehlen. |
| 25-26 / 22-25 | Auto-Loop, halbieren, verdoppeln, Reloop, SHIFT In/Out | **MIDI fehlt.** Engine-/GUI-Funktionen vorhanden; Loop-Noten auf Kanaelen5/6 fehlen. |
| 25 / 22 | SHIFT+AUTO: Auto4 | **Teilweise.** Quelle beschreibt vier Takte. Vorhandene Auto-Loops sind beatbasiert, kein ausdruecklicher Vier-Takte-Aufruf. Nicht als vier Beats implementieren; Umgang mit Taktwechseln festlegen. |
| 26 / 26,28 | Browse/Library, schnelle Auswahl, Back/Panel | **MIDI/Navigation fehlt.** Suche, Auswahl, Paging und Tabs vorhanden; Encoder, Druck, Shift-Schnellwahl sowie klarer Fokus-/Panelpfad fehlen. |
| 26 / 27 | LOAD A/B | **MIDI fehlt.** Selektierten Track laden funktioniert in der GUI; globale LOAD-Noten2/3 fehlen. Bestehenden Schutz spielender/beschaeftigter Decks erhalten. |
| 26 / 27 | LOAD-Doppeldruck: Instant Doubles | **S7 implementiert.** Bis350ms am Steuerthread; geteilter Track, Softwareposition/Basistempo/Grid/Keylock/Playzustand. Erster Druck normaler Library-LOAD, zweiter ersetzt nur dessen eigenen Job; laufendes Ziel und Konflikte geschuetzt. Ziel-Loops/TempCue zurueckgesetzt, Mixer erhalten. Details im GUI-Inventar, native Abnahme offen. |
| 26 / 29 | Load Prep / Open Prep | **App/MIDI fehlt.** Prepare ist aktuell Analyse-/Metadatenansicht, keine kuratierte Vorbereitungsqueue. Hinzufuegen und Oeffnen brauchen eine eigene persistente Liste. |
| 26 / 30-31 | Vier Sampler-Pads, SHIFT Stop, Sample-Pegel | **App/MIDI fehlt.** Kein Sampler mit vier Slots, begrenztem Audio-Mix und Pegelregelung. |
| 26 / 32 | Situationsabhaengige Hardware-Pegelanzeigen | **Teilweise.** LEDs zeigen derzeit A/B-Pre-Fader-Peaks. Das beschriebene Umschalten zwischen Pre-Fader-Cue und summiertem Post-Fader-Mix je nach Fader/Cue fehlt. Bestehende Messwerte sind wiederverwendbar. |
| 27 / Vorderseite4 | Split Cue | **App/MIDI fehlt.** Unabhaengiges Pre-Fader-Cue und Monitor-Gain/Mix inklusive Profilpersistenz funktionieren bereits. Es fehlt Mono-Cue links / Mono-Master rechts samt globaler Note28. |

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
Regressionen, passenden Dokumentationsstand und unabhaengigen Review.
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
| S8 Prepare-Queue | Persistente kuratierte Track-ID-Liste und LoadPrep/OpenPrep, getrennt vom Analyse-Tab. | Deduplizierung, Reihenfolge, Entfernen, Missing/Relocation und Neustart; keine unerwartete Analyse oder Wiedergabe. |
| S9 Erweiterte Jog-Navigation | SHIFT-Suche, gehaltenes Vor-/Zurueckspulen und Loop-In/Out-Editmodus. S2/S3. | Anfang/EOF, Geschwindigkeit/Modusprioritaet, Release/Modifierwechsel, Loops Mindestlaenge, Sourcewechsel und Scratch-Konflikte. |
| S10 Split Cue und Meterpolitik | Mono-Cue links/Mono-Master rechts sowie handbuchgerechte Hardwaremeter. | Stereo-/Mono-Summen und Pegel, Clip, Cue/Fader-Kombinationen und MASTER-Isolation; Mehrkanalrouting, gespeicherter Split-Zustand mit Profilmigration, native Kopfhoererprobe. |
| S11 Censor | Reverse mit separater normaler Zeitlinie, S2/S5/S9. | Samplegenauer Reverse, Rueckkehrzeit bei verschiedenen Tempi, Pause/EOF/Loop/Sync/Keylock, Release/Disconnect; Allokations-/Lockfreiheit und Callback-Frist messen. |
| S12 Musikalische FX-Zeitbasis | Beat-Unterteilungen, Tap und Hold-Reset; bestehendes Delay erhalten. | BPM/Tempo- und Gridwechsel, Tap-Ausreisser/Timeout, Rampen, Persistenz; keine Klicks oder unbeschraenkten Echtzeitpuffer. |
| S13 Drei FX-Slots und Auswahl | Begrenzte Slots je Deck, serieller/paralleler Signalvertrag, FX1-3/Shift-Pegel/-Auswahl. S12. | Vorab unterstuetzte Effekttypen und Routing festlegen; alten Delayzustand migrieren, Bypass/Fahnen/Parameter und CPU-/Speicherbudget; bisheriges Controller-Mapping nicht still veraendern. |
| S14 Vier-Slot-Sampler | Offline vorbereitete Samples, bounded Mixer, Play/Stop, Gesamtpegel und GUI. | Polyphonie/Restart-/OneShot-Vertrag festlegen; keine Datei-/Speicherarbeit im Callback, Peaks/Routing, Deck-Isolation, Stop/Disconnect und Neustart. |
| S15 Pad-Modus und Gesamtqualifikation | Nur konkret ausgewaehlte App-Modi, S4/S14; weitere Serato-Modi gesondert entscheiden. | Modus/LED/Fokus konsistent, keine verlorenen Releases bei Wechsel; alle Controller-Elemente A/B nach Testmatrix, reale Tracks, Latenz/Langzeit und Recovery messen. |

Gemeinsamer nativer [Hardware-Testplan S1-S15](MC4000_HARDWARE_TEST_PLAN.md).
Nach unzuverlaessigen QWEN-/DeepSeek-Implementierungsversuchen gilt der
autorisierte Fallback: S2-S6 selbst fertigstellen. Separat wurde S7 mit vom
Nutzer autorisiertem GPT-6.1 Sol umgesetzt. S8-S15 bleiben offen; der
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
