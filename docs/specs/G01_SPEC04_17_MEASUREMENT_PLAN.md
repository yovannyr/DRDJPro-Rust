# G01 – Messplan für MIDI und Beatgrid-Sync

Stand 08.11.2026. Grundlage: [Spec 4](4-midi-controller/spec.md), [Spec 17](17-beat-grid-sync/spec.md), [MIDI-Abgleich](G01_SPEC04_MIDI_AUDIT.md), [Sync-Abgleich](G01_SPEC17_SYNC_AUDIT.md), [G01-Messkonzept](G01_DECISIONS_AND_MEASUREMENT.md) und ein Entwurf von `deepseek-v4-flash-sovereign`. Der Entwurf wurde gegen den Code korrigiert: Quantisierung und MC4000-LED-Ausgabe existieren; mehrere gleichzeitig nutzbare Controller, generisches MIDI-Learn und ein Track-Tap sind nicht vollständig nachgewiesen. **Alle 16 Messungen sind geplant, nicht ausgeführt.**

Geräte, Anschluss, Treiber, Betriebssystem, Commit, Release-Build, Samplerate, Puffer und Last festhalten. Eingangsereignis und physische Ausgabe mit einem gemeinsamen Referenzzeitgeber messen; interne Zeitstempel und Headless-Tests ergänzen die Messung, ersetzen aber keine End-to-End-Gerätelatenz. Daten, Fehlversuche und Ausreißer dokumentieren; Stichproben und unklare Begriffe vorab festlegen.

## Spec 4 – MIDI Controller

| SC | Messstrecke und Originalziel | Voraussetzung / Status |
|---|---|---|
| 001 | Physisches Verbinden bis zur angezeigten Erkennung; innerhalb von 2 Sekunden. Geräte und wiederholte Verbindungs-/Trennzyklen dokumentieren. | MC4000-Pfad vorhanden; reale Messung und generische Geräteabdeckung offen. |
| 002 | Physischer MIDI-Eingang bis zur konkreten DJ-Funktionsreaktion; unter 20 ms. Visuelle und akustische Ausgänge getrennt messen. | Gemeinsamer Messaufbau und physische Messung fehlen. |
| 003 | Vorab definierte repräsentative Liste verbreiteter Controller ohne Customizing testen; mindestens 95 % funktionieren mit Presets. Geräteliste, Nenner und Fehlfunktionen ausweisen. | Geforderte generische Presets und repräsentativer Gerätepool nicht nachgewiesen. |
| 004 | Nutzertest von Start des Learn-Vorgangs bis gespeicherter und funktionierender Zuweisung; unter fünf Minuten pro Control. Verschiedene Control-Typen abdecken. | MIDI-Learn ist nicht als vollständige Funktion nachgewiesen. |
| 005 | Kontrollierte Eingangsfolge von mindestens 1000 Nachrichten/s je Gerät; versandte, verarbeitete und ausgelassene Ereignisse sowie Lags zählen. | Reproduzierbarer Lastgenerator, Messung und Definition von Lag fehlen. |
| 006 | Zustandsänderung der App bis zur tatsächlichen LED-Reaktion messen; innerhalb 50 ms. Verschiedene LED-Zustände prüfen. | LED-Pfad für MC4000 existiert; physische Zeitmessung fehlt. |
| 007 | Mindestens zwei Geräte gleichzeitig mit sich überschneidenden Aktionen betreiben; Isolation und fehlende Konflikte prüfen. | Mehrgerätebetrieb und geeignete Geräte sind nicht vollständig nachgewiesen. |
| 008 | Mehrere benutzerdefinierte Mappings speichern, App neu starten und jede Zuordnung samt Modus und Kurve prüfen. | Freie Mapping-Persistenz nicht als Gesamtfunktion belegt. |

## Spec 17 – Beat Grid & Sync

| SC | Messstrecke und Originalziel | Voraussetzung / Status |
|---|---|---|
| 001 | Aktivierung bis zur abgeschlossenen BPM-Synchronisierung messen; innerhalb 500 ms. Verschiedene Ausgangstempi und Pitch-Grenzen getrennt auswerten. | Syncpfad vorhanden; definierte Messung und Lastfälle fehlen. |
| 002 | Nach Sync die tatsächlichen Beatpositionen beider Audioausgaben gegen gemeinsamen Referenztakt vergleichen; innerhalb ±5 ms. | Annotierte Signale und Ausgabe-Messaufbau fehlen. |
| 003 | Mindestens 30 Minuten kontinuierliche Wiedergabe; zeitlichen Phasenfehler über den gesamten Lauf erfassen, nicht nur am Ende. Ziel innerhalb ±10 ms. | Langzeitmessung auf dokumentiertem Interface fehlt. |
| 004 | Grid-Offset mit 0,01-s-Schritten über den spezifizierten Bereich einstellen, speichern und Position gegen Soll vergleichen; Präzision 0,01 Sekunden. | Editor vorhanden; vollständiger UI-/Speicher-Messlauf fehlt. |
| 005 | Unterschied zwischen Deck-BPM bei wiederholten Änderungen beobachten; Anzeige aktualisiert sich alle 100 ms. | Spezifizierte Differenzanzeige und ihre Messung nicht nachgewiesen. |
| 006 | Beatereignis bis sichtbarem Indikator messen; unter 16 ms bei 60 FPS. | Geforderte Anzeige und tatsächliches Frame-Timing fehlen als Nachweis. |
| 007 | Aktionen auf verschiedenen Phasen und Gridunterteilungen auslösen; tatsächliche Einrastposition gegen Beat vergleichen; innerhalb ±1 ms. Interne Frameberechnung und Audioausgabe getrennt auswerten. | Quantisierung existiert; vollständiger Timingnachweis fehlt. |
| 008 | Mindestens vier präzise Taps bei verschiedenen bekannten Tempi; berechnetes Track-BPM gegen Referenz vergleichen; innerhalb ±0,5 BPM. | Track-Tap nicht nachgewiesen; FX-Tap ist kein Ersatz. |

Alle Kriterien bleiben bis zu den beschriebenen Prüfungen offen. Zusätzlich gelten die vollständigen Originalformulierungen und Testfälle der jeweiligen Spec.
