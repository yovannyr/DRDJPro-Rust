# G01 – Messplan für Wellenform und Performance Pads

Stand 08.11.2026. Grundlage: [Spec 7](7-advanced-waveform/spec.md), [Spec 16](16-performance-pads/spec.md), [Wellenform-Abgleich](G01_SPEC07_WAVEFORM_AUDIT.md), [Pad-Abgleich](G01_SPEC16_PADS_AUDIT.md) und ein Entwurf von `deepseek-v4-flash-sovereign`. Modellannahmen wurden anhand des Codes korrigiert: Eine dreibandige Frequenzfarb-Heuristik, zwei MC4000-Padmodi und Hot-Cue-Persistenz existieren bereits. **Keine Messung ist damit als bestanden nachgewiesen.**

Pro Lauf Commit, Release-Build, Plattform, Daten, GPU, Bildschirm, Audiointerface, Samplerate, Buffer und Eingabegerät dokumentieren. UI-Ereignis, Renderzeit und tatsächlich angezeigten Frame unterscheiden; bei Audio die physische Eingabe bis zum Ausgang mit gemeinsamem Zeitbezug erfassen. Bestehende Unit-/Headless-Tests ergänzen die Hardware- und Sichtprüfung, ersetzen sie aber nicht. Stichprobe und Behandlung von Ausreißern vorab festlegen.

## Spec 7 – Advanced Waveform

| SC | Messstrecke und ursprüngliches Ziel | Voraussetzung / Status |
|---|---|---|
| 001 | Typische vierminütige Tracks und Standardauflösung: Ladeauftrag bis zur sichtbaren Wellenform; innerhalb von 3 Sekunden. | Messung auf definierter Hardware fehlt. |
| 002 | Vierminütige Tracks, FFT 2048: Auftrag bis fertig generiertes Spektrogramm; innerhalb von 10 Sekunden. | Spektrogrammpfad nicht nachgewiesen. |
| 003 | Playhead während Wiedergabe anhand sichtbarer Frames verfolgen; flüssige 60 FPS. | Messlauf und sichtbare Frameausgabe fehlen als Nachweis. |
| 004 | An mehreren bekannten Stellen der Wellenform klicken; Zielzeit mit tatsächlicher Position vergleichen; innerhalb ±100 ms. | Ein-/Ausgangsbeobachtung und Messung fehlen. |
| 005 | Referenzannotierte Bass-/Mitten-/Höhenabschnitte gegen angezeigte Frequenzfarben auswerten; mindestens 85 % korrekt. | Dreifarbige Heuristik existiert; annotiertes Corpus und Bewertung fehlen. |
| 006 | Gridmarker gegen unabhängig annotierte tatsächliche Beats vergleichen; innerhalb ±50 ms für 90 % der Tracks. Automatisches und manuell korrigiertes Grid getrennt ausweisen. | Automatische Beatdetektion und Corpus sind nicht vollständig belegt. |
| 007 | Zwischen allen geforderten Visualisierungsmodi wechseln; sichtbarer Wechsel innerhalb 200 ms ohne Unterbrechung der Wiedergabe. | Alle geforderten Modi und Audio-/UI-Messung fehlen als Nachweis. |
| 008 | Hover-Tooltips während Interaktion anhand angezeigter Updates messen; mindestens 30 FPS. | Spec-konforme Zeit-/Energie-Tooltips und Messung nicht nachgewiesen. |

## Spec 16 – Performance Pads

| SC | Messstrecke und ursprüngliches Ziel | Voraussetzung / Status |
|---|---|---|
| 001 | Physischer Pad-Druck bis Audioausgang über Interface, Puffer und Gerät messen; unter 10 ms. Interne Zeitstempel nur ergänzend erfassen. | Hardwaremessung fehlt. |
| 002 | Roll-Loop-Grenzen mit Referenzsignalen analysieren und abhören; sampleperfekte Loops ohne hörbare Klicks. | Roll-Padmodus nicht nachgewiesen. |
| 003 | Beat-Jump-Zielposition gegen Grid und tatsächliche Ausgabe vergleichen; innerhalb ±5 ms. | Geforderte Pad-Distanzen und Messung nicht vollständig belegt. |
| 004 | Pad-Eingabe bis zur sichtbaren Reaktion messen; unter 16 ms bei 60 FPS. | Native Sicht-/Gerätemessung fehlt. |
| 005 | Alle acht Slicer-Positionen gegen das Grid prüfen; perfekt quantisiert gemäß Spec. Vorab festlegen, wie die Genauigkeit in Frames nachgewiesen wird. | Slicer-Padmodus nicht nachgewiesen. |
| 006 | Mindestens zehn Pad-Trigger pro Sekunde unter dokumentierter Last erzeugen; zeitliche Ausgabe, verlorene Eingaben und Audioaussetzer erfassen. | Reproduzierbarer Lastlauf fehlt; „ohne Lag“ benötigt eine vorab festgelegte Messinterpretation. |
| 007 | Alle geforderten Moduswechsel von Eingabe bis wirksamem Zustand messen; innerhalb 100 ms. | Hot-Cue- und Sampler-Modus existieren; weitere vier Modi fehlen als Nachweis. |
| 008 | Hot Cues mit Position, Farbe und Namen ändern, persistieren, App neu starten und vergleichen; kein Datenverlust. Fehler-/Abbruchfälle separat prüfen. | Persistenzpfad und automatisierte Tests existieren; spezifizierte Gesamtprüfung bleibt offen. |

Alle 16 Erfolgskriterien bleiben bis zu dokumentierten Messungen offen. Ein einzelner vierminütiger Track genügt nicht für alle Kriterien; Testdaten und Wiederholungen sind je Messung passend festzulegen.
