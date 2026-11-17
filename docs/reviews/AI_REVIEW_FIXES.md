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

## Noch zu bearbeitende Kritikpunkte

- Legacy-Trackanalyse mit angenommenem BPM/Grid/Key und unehrlicher Batch-Fortschreibung.
- Mock-Ergebnisse und vorgetäuschte Aktionen im Legacy-Track-Prep-Modell.
- Harmonie-Vorschlag ohne Tonartdaten im regelbasierten Neural-Sync-Modell.
- Nicht eingebundener Scheduler-Prototyp mit simulierten Fortschrittsmeldungen.
- Unbelegte Vollständigkeits- und Prozentangaben in der Serato-Gap-Analyse.

Der echte Controller-/AudioAnalyzer-Pfad ist davon getrennt. Visuelle, manuelle,
Hardware- und Hörabnahmen werden nicht aus diesen Softwaretests abgeleitet.
