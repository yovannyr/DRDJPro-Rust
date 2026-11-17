# Korrekturen zur KI-Bestandsaufnahme

Stand: 2026-11-17. Ausgangsbasis: PR-9-Merge `466ef36`.

Die Nutzeranfrage beauftragt die Korrektur der überprüften Kritikpunkte aus
`AI_INTEGRATION_PLAN.md`. Dessen spätere Backend-Spikes und Modellintegration
sind Planungsinhalt und keine bereits implementierten Fähigkeiten.

## Slice 0a — ehrliche Stem-Separation

`src/stems.rs` gibt keine unveränderten Mixkopien als getrennte Instrumente
zurück. BandSplit und PhaseSplit liefern `StemError::NotImplemented`, MLBased
ohne Backend `ModelNotLoaded`, Unknown einen Parameterfehler. Fehlversuche
setzen Fortschritt und Abschlussstatus zurück; ein Cache-Eintrag macht ein
nicht implementiertes Verfahren nicht verfügbar. Die feste Qualität 0.75
und tote private Separations-/Hash-Hilfen wurden entfernt.

Manuell gelieferte Stem-Daten, Mischung und der öffentliche Cache-Vertrag
bleiben bestehen. Es wurde kein neuer DSP-Algorithmus eingeführt und kein
Audio-Callback geändert; der Fehlerpfad kopiert keine Audiodaten und erzeugt
keine Worker oder Modelle. Echte automatische Stems bleiben offen.

Gezielte Prüfung: 24 Stem-Tests bestanden, einschließlich aller nicht verfügbaren
Verfahren, Fehlerstatus, unverändertem Cache und manueller Schlüssel-Isolation.
Abschlussprüfung: `cargo +1.99.1 test --lib --locked` mit 1.718 bestandenen
Tests und `cargo +1.100.0 clippy --lib --locked --target
x86_64-pc-windows-msvc -- -D warnings` grün. Formatierung und Diff-Prüfung grün.
Unabhängiges statisches Review: GPT-6.1 Sol, medium; nach Korrektur des
veralteten Hash-Tests und der Modulbeschreibung keine offenen Befunde.

## Slice 0b — Analyse- und Sync-Verträge

Die unverbundene Legacy-Trackanalyse liefert `AnalysisError::NotImplemented`
statt BPM 128, Key 8A und künstlichem Grid. Ein Batch stoppt mit Fehler ohne
Erfolgsfortschritt. Vorhandene manuelle Analyse-, Cue- und Grid-Daten bleiben
erhalten. `mark_complete` übernimmt nur gelieferte BPM/Key-Werte, erfindet keine
Confidence oder Energie und erklärt diese Daten nicht automatisch für zuverlässig.

Im Legacy-Prep-Vertrag liefern automatische Key-Erkennung, Reanalyse und
Speichern/Laden explizite Fehler. **API-Änderung:** `detect_key` liefert jetzt
`Result<KeyDetection, PrepError>`; bisherige Aufrufer müssen den Fehler behandeln.
Es gibt keine produktiven Aufrufer im Repository. Manuelle Tonartkorrekturen
aktualisieren tatsächlich die Tonart; ein bekannter Key bedeutet nur `KeyDetected`.
Beat-/Taktmarker erzeugen keine angeblich erkannten Phrasen mehr.

Neural Sync bleibt ein regelbasiertes Modell für gelieferte Annotationen. Ohne
Tonartdaten gibt es keinen Harmonie-Vorschlag; Vorschläge erhöhen keinen
Erkennungszähler. Energie-Scores werden nicht als Phrasenqualität ausgewiesen.
Nicht implementierte Settings-Persistenz liefert Fehler ohne Zustandsänderung.
Der nicht eingebundene Scheduler mit simuliertem Fortschritt wurde entfernt.

Die Änderungen betreffen keine Decoder, DSP-Verfahren oder Audio-Callbacks.
Fehlerpfade starten weder Inferenz noch Worker und kopieren kein PCM. Kein neues
Paket oder Modell wird heruntergeladen. Tests prüfen Fehler ohne erfundene Daten,
den Erhalt manueller Daten, fehlende Datei-Schreibeffekte und korrekte Metriken.
Unabhängiges statisches Review: GPT-6.1 Sol, medium; einschließlich Folgeprüfung
der Confidence-Korrektur und des zusätzlichen Datenerhalt-Tests ohne Blocker.

## Slice 0c — belegbarer Funktionsstand

Die Serato-Gap-Analyse trennt jetzt `Real mit Test`, `Hülle/Stub` und `Fehlt`
pro begrenztem Vertrag. Die unbelegten 89–93 %, pauschale Vollständigkeit,
erfundene Tests und falsche Bibliotheks-Gleichsetzungen wurden entfernt.
Specs 26–31 sind mit Quellen und ihren offenen Grenzen fortgeschrieben;
Partnerzugänge, native Geräte und der Stand in `main` bleiben getrennt.
Alle 25 lokalen Linkziele sind vorhanden. Unabhängiges statisches Review:
GPT-6.1 Sol, medium; keine blockierenden Befunde.

Die überprüften Slice-0-Kritikpunkte sind damit korrigiert. Die späteren
Backend-Spikes, Modelle und automatische Stem-/Phrasenerkennung bleiben offen.

## Finale automatisierte Prüfungen (Slices 0b/0c)

- `cargo +1.99.1 test --workspace --lib --bins --locked`: 2.360 bestanden,
  6 bestehende Tests ignoriert, 0 fehlgeschlagen. Darin 1.720 Root-Library- und
  579 Root-Binary-Tests einschließlich der unveränderten realen Analyse-,
  Controller-, Playback- und Keylock-Verträge. Kein vollständiger neuer Lauf
  der Integrationstests oder nativen Prozess-Probes in diesem Durchgang.
- `cargo +1.100.0 clippy --workspace --locked --all-targets --target
  x86_64-pc-windows-msvc -- -D warnings`: grün, keine Warnungsunterdrückung.
- `cargo +1.100.0 fmt --all -- --check` und `git diff --check`: grün.
- Dokumentation: 25 lokale Linkziele vorhanden; benannte Testfunktionen im
  unabhängigen Review abgeglichen. Keine Serato-Paritätsmessung durchgeführt.

Gemeinsames vorhandenes Target-Verzeichnis, ein Build-Job, keine neue Dependency,
kein Release-Build und kein Modell-Download. Nach den Prüfungen rund 47,5 GiB
auf C: frei (Momentaufnahme, keine zugesicherte Speicherobergrenze).

Der echte Controller-/AudioAnalyzer-Pfad ist davon getrennt. Visuelle, manuelle,
Hardware- und Hörabnahmen werden nicht aus diesen Softwaretests abgeleitet.

## Fortschreibung — A0

Nach Abschluss und Main-Integration von Slice 0 folgt der isolierte
[CPU-Backend-Spike A0](../adr/ML-BACKEND.md). Seine Messungen und Prüfungen sind
separat dokumentiert; die obigen Zahlen gehören zu den früheren Kritik-Fixes.
Optionale Runtime-Adapter und ein synthetischer Testgraph bedeuten weiterhin
keine produktive Stem-, Beat- oder Phrasenerkennung. A1 ist noch nicht gestartet.
