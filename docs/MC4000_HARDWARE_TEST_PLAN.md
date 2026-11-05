# MC4000: gemeinsamer Hardware-Testplan S1-S15

## Umfang und Protokoll

Dieser Plan prueft den [Implementierungsplan](MC4000_IMPLEMENTATION_PLAN.md)
am echten Denon DJ MC4000. Nur bereits implementierte Slices ausfuehren;
ein offener Slice ist **nicht testbar**, nicht automatisch bestanden.
Automatisierte Tests, Review und native Hoer-/Bedienabnahme getrennt notieren.
S1 und physisches/virtuelles Scratchen wurden vom Nutzer bereits bestaetigt.
Die restlichen Abnahmen sind bis zur tatsaechlichen Ausfuehrung offen.

S2-S6 sind implementiert; S7 Instant Doubles ist als eigener Folgeschritt
implementiert; S8 ergaenzt die persistente Prep Queue, S9 Navigation/Loop-Jog.
S10 ergaenzt Split Cue/kontextuelle Hardwaremeter, S11 momentanes Censor.
S12 ergaenzt musikalische Delay-Zeitbasis/Tap/Reset, S13 drei serielle FX-Slots.
S14 ergaenzt den Vier-Slot-Sampler, S15 die zwei App-Padmodi und LED-Zustaende.
S15 ist softwareseitig implementiert; die native Gesamtqualifikation ist offen.
Native Abnahmen fuer S2-S15
bleiben bis zur tatsaechlichen Ausfuehrung offen.

Fuer jeden Fall festhalten: Commit, Datum, Windows-/Treiberstand, Audiogeraet,
Samplerate, Ausgangspaare, Tracks, Ergebnis PASS/FAIL/NOT IMPLEMENTED,
Fehlermeldung und Wiederholungsschritte. Keine persoenlichen Dateipfade oder
Trackinhalte in oeffentlichen Testberichten veroeffentlichen.

## Vorbereitung

1. Bibliotheksbackup erstellen und ein isoliertes Testprofil verwenden.
2. MC4000 anschliessen; MASTER 1/2, Kopfhoerer-Cue 3/4 waehlen.
3. MASTER-/Kopfhoererpegel zuerst niedrig. Keine absichtlichen Vollpegeltests
   ueber angeschlossene Lautsprecher; digitale Clip-Pruefung bei stummen Ausgaengen.
4. Zwei gut bekannte Tracks mit bestaetigten BPM/Beatgrids, einen Track mit
   anderem Tempo, einen 3/4-Track und einen Track mit Taktwechsel bereithalten.
5. Fuer Librarytests > eine Seite Tracks, Suchfilter, Crates und einen fehlenden
   Testtrack verwenden. Fuer Sampler vier kurze unterschiedliche Samples.
6. A/B jeweils laden, aber pausiert lassen; EQ neutral, Filter aus, FX bypass,
   Pitch0%, Sync aus, keine gehaltenen Pads. Aktuellen Zustand dokumentieren.

## Testmatrix

Jede deckbezogene Zeile auf **A und B** pruefen. Das jeweils andere Deck darf
sich ohne ausdrueckliche Sync-/Klon-/Masteraktion nicht aendern.

| ID / Slice | Handlung | Erwartetes Ergebnis |
| --- | --- | --- |
| S1-01 Trim | Regler von Mitte nach links/rechts und zurueck, bei laufendem Track | GUI-Trim folgt, Absenkung/Anhebung hoerbar, Mitte0dB; Fader/Tempo/Position unveraendert. |
| S1-02 EQ | High/Mid/Low einzeln bewegen und zur Mitte zurueck | Nur gewaehltes Band/Deck aendert Klang; Mitte neutral; keine dauerhaften Klicks/Aussetzer. |
| S1-03 Filter | Mitte->links->Mitte->rechts->Mitte | Low-Pass / aus / High-Pass / aus; MASTER und unabhaengiges Cue folgen demselben Deck-DSP. |
| S2-01 Pitch | Fader langsam ueber gesamten Weg, dann kleine Schritte um Mitte | GUI-/BPM-/Pitchwerte folgen zusammenhaengend, Mitte neutral, kein Sprung durch halbe MIDI-Paare. |
| S2-02 Pickup | GUI-Pitch aendern, Hardwarefader entfernt halten und langsam bewegen | Bis zum Erreichen/Ueberqueren des aktuellen Zielwerts kein Temposprung; danach direkte Steuerung. |
| S2-03 Bereich | SHIFT+KEYLOCK mehrfach, Hardwareposition beibehalten | Bereich zyklisch gemaess GUI; Pickup neu bewaffnet, kein unerwarteter Keylock-/Pitchwechsel. |
| S2-04 Bend | +/- druecken, halten, loslassen; beide Richtungen gegeneinander | Nur voruebergehende Tempokorrektur; nach Release wieder Grundtempo; keine eingerastete Korrektur. |
| S2-05 SHIFT | SHIFT halten/loslassen, dann unshifted Taste bedienen | Keine haengende Shiftfunktion; normale Taste fuehrt wieder normalen Befehl aus. |
| S3-01 Sync | Beide Tracks mit geeigneten Grids spielen; SYNC am Follower, SHIFT+SYNC aus zweimal | Tempo/Phase angleichen; Aus idempotent, kein erneutes Aktivieren; fehlende Voraussetzungen melden Fehler. |
| S3-02 Loop | Auto, halbieren, verdoppeln, Reloop | Angezeigte/hoerbare Looplaenge korrekt; Reloop aktiviert letzte Grenzen; anderes Deck unbeeinflusst. |
| S3-03 Manuell | SHIFT+1/2 In setzen, SHIFT+x2 Out spaeter setzen | Gueltiger Loop, ungueltiges Out vor In explizit abweisen; keine Ausserhalb-Track-Grenzen. |
| S3-04 Auto4 | SHIFT+AUTO bei4/4,3/4 und Taktwechsel | Vier Takte laut dokumentiertem Vertrag, nicht pauschal vier Beats; ununterstuetzte Metren explizit melden. |
| S4-01 Pads | Leeres Pad setzen, besetztes Pad bei Wiedergabe abrufen | Slots1-4 und passende LEDs; GUI/Marker springen korrekt; Slots5-8 unveraendert. |
| S4-02 Preview | Bei Pause belegtes Pad halten, loslassen; wieder halten und PLAY druecken | Preview waehrend Hold; Release-Verhalten wie dokumentiert; PLAY uebernimmt ohne spaetere Zwangspause. |
| S4-03 Delete | SHIFT+belegtes Pad, danach Neustart | Nur Zielcue geloescht, LED/Marker aus; andere Cue-Slots erhalten, Loeschung persistent. |
| S5-01 TempCue | Pause, per Jog positionieren, CUE setzen, dann Hold/Release | Temporaerer Punkt getrennt von gespeicherten Cues; Preview und Rueckkehr/Pause am richtigen Punkt. |
| S5-02 PlayLatch | TempCue halten, PLAY druecken, beide loslassen | Wiedergabe laeuft weiter; kein Ruecksprung oder haengender Hold. |
| S5-03 Start/Stutter | SHIFT+CUE; danach temporaeren Punkt setzen und SHIFT+PLAY wiederholen | Trackstart0 bei erstem Befehl; Stutter ab temporaerem Punkt, nicht aus gespeichertem Hot-Cue-Slot. |
| S6-01 Browse | Encoder langsam/schnell mit SHIFT, Seite wechseln und Suchfilter aktivieren | Stabile Reihenfolge/Auswahl ueber Seiten; sichtbarer Track folgt Auswahl, keine verlorenen oder doppelten Spruenge. |
| S6-02 Focus | Encoderdruck, Back und SHIFT+Back bei Library/Panel/Texteditor | Druck wechselt Tracks/Crates bzw. verlaesst Suchfokus; Back zu Crates; SHIFT+Back naechster App-Librarytab. Offene Editoren geschuetzt, kein unbeabsichtigtes Bearbeiten von Textwerten. |
| S6-03 LOAD | A/B LOAD fuer ausgewaehlten Track; laufendes/beschaeftigtes Ziel und Missing-Track | Pausiertes Ziel laedt richtigen Track; Schutz/Fehler sichtbar, anderes Deck bleibt erhalten. |
| S7-01 Doubles | Gueltigen Librarytrack waehlen; LOAD A/A bzw. B/B bis350ms und danach; A/B, dritter Druck, anderer Controllerbefehl zwischen den Drucken | Erster Druck normal sofort; zweiter bis350ms Klon des anderen Decks; ausserhalb normal. Andere Befehle/Recovery brechen Geste ab. Laufendes Ziel geschuetzt, fremder Ladejob erhalten. |
| S7-02 CloneState | Quelle mit Position/Pitch/Keylock abspielen oder pausieren; auf pausiertes Ziel klonen, auch nach fertiggestelltem ersten LOAD | Ziel uebernimmt Softwareposition/Basistempo/Grid/Keylock/Playzustand, Loop aus und TempCue zurueckgesetzt, Mixer/EQ/FX unveraendert. Quelle unveraendert; Scratch/Loop/Nudge/Sync/gehaltene oder geplante Cues sowie ausstehende Vorbereitung werden abgewiesen. Hoerprobe und Zeitversatz messen. |
| S8-01 Prepare | Track mit LOAD PREP hinzufuegen, erneut hinzufuegen, SHIFT+LOAD PREP oeffnen; GUI Up/Down/Remove | Eigener Prep-Queue-Tab, keine Analyse starten; keine Duplikate; kuratierte Reihenfolge, Filter behaelt Ordnung. LOAD behaelt Eintrag, Remove loescht keine Datei. |
| S8-02 Restart | Queue bearbeiten, Track fehlen lassen/relokalisieren, Neustart | Queue persistent; stabileIDs, fehlende Tracks erkennbar, kein automatisches Laden/Abspielen. |
| S9-01 Search | SHIFT+Jog; SHIFT+Bend32/33 halten/loslassen, beide Tasten, SHIFT loslassen, Fokus/USB wechseln; A/B pausiert/laufend | Jog50ms/Einheit, gehaltene Suche8s/s additiv zum Softwareplayhead, >100ms Tick-Gap beendet ohne Nachholen. Pause bleibt Pause, EOF stoppt; kein versehentliches Scratch/Bend. Native Hoer-/Latenzprobe noetig. |
| S9-02 LoopEdit | AUTO JOG Off/In/Out; bei aktivem Loop Hardware IN/OUT druecken, beruehren/drehen; AUTO verlassen und Source/Transport wechseln | Edit hat Vorrang vor SHIFT, nur gewaehlte Grenze1ms/Einheit, Sample-Rundung und mindestens ein Frame. Aktiver ausgeschlossener Playhead geht zu In; sonst Transport erhalten. Modus im Caption, unmittelbare Freigabe bei Panelwechsel; Slot-Speichern explizit. |
| S10-01 Split | CueB/MasterA, Split-Schalter/GUI ein/aus, Mix0/50/100%, Gain und Mono-/Stereo-Tracks | Split: gemitteltes Cue mono links, Master mono rechts; Mix0/100 schaltet jeweils andere Seite stumm, Mitte halbes Gewicht je Seite.5ms Blend, aus vorhandener Stereo-Mix; MASTER unveraendert.3/4 reserviert auch ohne aktive Cue-Auswahl. |
| S10-02 Meter | Fader beide unten/einer oben, Cue ein/aus, Deck pausiert/laufend/scratch; Audiofortschritt stoppen und fortsetzen | Beide unten: Pre A/B; sonst Master auf beiden ausser laufendem Cue mit eigenem Fader unten. Pausiertes Cue zurueck zu Master. Aktuelles50ms Fenster, kein GUI-Clip-Hold; bei weiterlaufendem UI und >100ms ohne Produzentenfortschritt dunkel, nach neuem Fortschritt wieder aktiv. Kein physischer MASTER-Einfluss. |
| S10-03 Profile | Split speichern, Neustart und Geraetewechsel2/4Kanaele | Profil korrekt wiederhergestellt; Konflikte/fehlendes Cuepaar explizit statt falscher Ausgabe. |
| S11-01 Censor | Laufend SHIFT+VINYL halten, dann loslassen | Rueckwaerts hoerbar, CENSOR sichtbar; Release/SHIFT-Release kehrt auf normal fortgelaufene Zeitlinie zurueck. Wiederholtes NoteOn startet nicht neu; neue Geste nach Abbruch erst nach Release. Eintritt/Austritt auf musikalisch stoerende Spruenge/Klicks pruefen; keine neue Ueberblendung zugesichert. |
| S11-02 CensorEdges | Bei anderem Pitch, nahe Anfang/EOF, mit Loop/Sync/Keylock | Anfang: Reverse stumm, normale Zeit laeuft; normales EOF stumm. Loop: beide Koepfe innerhalb Grenzen; Keylock: vorbereiteter Pitch bleibt. Controller stoppt Sync, kein automatischer Wiederanlauf. Pause/Seek/Pitch/Quelle beenden Censor, spaetes Release startet nichts. Scratch/Nudge/pending Cue und Jog-Konflikte explizit. Focus/Disconnect/Overflow pruefen. |
| S12-01 BeatFX | Beatunterteilungen aendern, Tempo/Beatgrid aendern | 1/16 bis4 Beats, gespeicherte lokale Grid-BPM mal effektivem Tempo; Timing folgt Steuer-Ticks. Fehlendes Grid/ausserhalb20-1000ms: erklaerter Bypass/FX-LED aus, spaeter automatische Rampen-Rueckkehr. CC2 waehlt wieder Millisekunden. Stereo/Rampen mit Musik auf Klicks pruefen. |
| S12-02 Tap | Regelmaessig tappen, Ausreisser/Timeout, Taste halten zum Reset | Ab3 kurzen Taps feste40-240BPM, Median bis4 Intervalle, Bounce<250ms ignoriert, Gap>1.5s startet neu. Hold>=1s setzt genau einmal auf Deck-BPM, ohne Grid expliziter Bypass. NoteOn0/NoteOff, GUI/HW-Besitz, Focus/Panel/Recovery und Profil-/Preset-Neustart pruefen. |
| S13-01 Slots | FX1-3 einzeln aktivieren, Pegel/SHIFT-Auswahl bewegen | Drei serielle Delay/LP/HP-Slots unabhaengig aktivieren. Legacy-Reglermodus pruefen, dann sichtbar auf Slot-Intensitaeten umstellen; SHIFT-Tasten waehlen Typ, SHIFT-Regler Delay-Feedback. LEDs folgen aktivem DSP, GUI zeigt alle Zustaende; anderes Deck unveraendert. |
| S13-02 Migration | Altes Delayprofil laden, Slots speichern/Neustart, Bypass | Alte Werte in Slot1, andere aus; alte Presets ebenso, neue Presets ganze Kette. Typ-/Bypass-Wechsel mit Musik auf Pegelspruenge testen; Delayhistorie bleibt bei Pause/Trackwechsel erhalten. Einstellungen nach Geraetewechsel erhalten, Historien neu. Speichergrenze64MiB/Deck; native Dauerlast/Feedback-Peaks pruefen. |
| S14-01 Sampler | Vier verschiedene Slots starten, gemeinsam triggern, SHIFT Stop | Note33-36 One-Shot/Restart (Release ohne Wirkung), Note40-43 Stop nur Zielslot. Max4 Stimmen, bis5ms Uebergaenge, kein Loop/Tempo-Sync. GUI- und MIDI-Besitz bei Disconnect/Overflow trennen; keine spaeten Starts nach Clear/Restore. Sampler-Pad-LEDs erst S15 qualifizieren. |
| S14-02 Level | Samplerpegel0/halb/voll waehrend Deckwiedergabe | CC26/GUI beeinflusst nur Samplebus; Decktransport/Crossfader/FX unabhaengig.25% Headroom pro Stimme; Master kann mit Decks/FX weiter clippen. Cue-only ohne Sampler, Master-Mix/Split rechts mit Sampler. Bank-Neustart/fehlende Datei/Geraetewechsel: neu laden, stets gestoppt. Auf Klicks und Dauerlatenz pruefen. |
| S15-01 PadMode | A/B: Note127 Kanal11/12 und GUI MC-Modus wechseln, auch waehrend HotCue-Hold, GUI-Co-Hold und Sampler-Playback; wiederholte NoteOn und spaete Releases | Hot Cues blau1 / Sampler lila3. Hardware-Pads: HotCue1-4 oder Sampler1-4, SHIFT Delete oder Stop. Wechsel gibt nur Hardware-Cue bzw. eigene Pad-Stimmen frei; GUI/anderes Deck bleiben erhalten. Gehaltene Pads erst nach echtem Release wieder aktiv. GUI-/Tastatur-HotCues bleiben unabhaengig. |
| S15-02 LEDs/Recovery/Unsupported | Sampler-Pads leer/bereit/laufend/ladend, Modes A/B getrennt, Fokus/USB/Overflow/Neustart; weitere Serato-Modi pruefen | Deck-Samplerpads off0/lila3/rot2, dedizierte Note33-36 LEDs ready127/sonst0. Recovery setzt HotCue-Modus, stoppt Hardware-Stimmen, erhaelt GUI-OneShots. Kein Roll/Slicer/Flip-Modus angeboten; keine unbelegten Funktionsbehauptungen. |

## Gemeinsame Recovery- und Regressionstests

Sampler-Sampleraten: Eine96kHz-Datei auf48kHz-Ausgabe laden bzw. als gespeicherte
Bank nach Geraetewechsel wiederherstellen: sichtbarer Fehler mit Quell-/Ausgaberate,
kein neuer Clip und kein Autoplay. Bei fehlgeschlagenem Ersatz bleibt der vorherige
Clip erhalten. Auf96kHz-Ausgabe muss dieselbe Datei ladbar und zunaechst gestoppt
sein. Downsampling ist bis zu einem bandbegrenzten Worker-Resampler nicht erlaubt.

Fuer **jede gehaltene Funktion** (Bend, Cue, HotCue, Scratch, Search, Censor)
separat ausfuehren:

1. Hold starten, Controller trennen, wieder verbinden: kein Stuck-Hold,
   keine automatische Wiedergabe eines zuvor pausierten Decks.
2. Hold starten, RECONNECT MIDI ausloesen; neues Handle und neue Eingabe
   duerfen keine alten Tastenereignisse wiederholen.
3. Source wechseln/entladen bzw. erlaubten GUI-Transport ausloesen:
   alte Eingabe darf nicht auf neuem Track weiterarbeiten.
4. GUI-Fokus/Ansicht wechseln und Fenster deaktivieren:
   Maus-/Keyboardholds sicher freigeben; Hardwareverhalten gemaess Vertrag.
5. Maus-Scratch mit gleichzeitigem Controller-Touch/Jog:
   bestehender Eingabebesitz bleibt sicher; Drag-Hand nur fuer akzeptierten
   Mausscratch, kein Cursorverlust nach Sekundaerbutton.
6. FX-/Mixerbewegung waehrend Libraryanalyse, Sortierung und Speichern:
   Audio stabil, Parameter richtiges Deck, keine blockierende GUI-/Audiowartezeit.

## Dauerlauf und Timing

- Mindestens30Minuten zwei Decks mit Cue, FX und wiederholter Librarynavigation.
  Aussetzer, CPU-/Speicherwachstum, Save-/Devicefehler und hoerbare Artefakte notieren.
- Scratch/Bend/Pad/Censor-End-to-End-Latenz mit geeigneter Aufzeichnung messen;
  Treiberpuffer, Samplerate, Messmethode, Stichprobenzahl und Maximum berichten.
  Subjektiv "schnell" ist keine gemessene Latenzgarantie.
- Software-Callback-/Allokationstests sind separate Nachweise. Hardwaretests
  ersetzen nicht automatisierte Reverse-/Sync-/Grenz-/Thread-Safety-Regressionen.

## Ergebnisvorlage

| Fall | Commit / Konfiguration | Ergebnis | Beobachtung / Reproduktion |
| --- | --- | --- | --- |
| Sx-yy | eintragen | PASS / FAIL / NOT IMPLEMENTED | eintragen |

Gesamtabnahme erst, wenn alle implementierten Zeilen und Recoverytests bestanden
und alle offenen Funktionen explizit als offen ausgewiesen sind. Kein
Produktfreigabehaken allein aufgrund eines erfolgreichen Builds.
