# G01 – Messplan für Mastering, DVS und Stems

Stand 08.11.2026. Grundlagen: [Spec 10](10-audio-mastering/spec.md), [Spec 11](11-dvs-control/spec.md), [Spec 12](12-stem-control/spec.md), [Bestandsabgleich](G01_ADVANCED_FEATURES_AUDIT.md) und ein Entwurf von `deepseek-v4-flash-sovereign`. Modellvorschläge wurden gegen die Specs geprüft; ein linearphasiger EQ, bestimmte Audio-Treiber-Backends oder konkrete Separationstools sind keine hier zusätzlich eingeführten Pflichtvorgaben. **Keine der 24 Messungen ist ausgeführt.**

Je Lauf Commit, Release-Build, Plattform, CPU/GPU, Interface/Treiber, Samplerate, Buffer, Testsignal bzw. lizenziertes Corpus, Referenz und Rohwerte dokumentieren. Zeitmessungen zwischen definierten Ein- und Ausgängen durchführen; Kontrollpfad-Zeitstempel ersetzen keine physische Messung. Qualität und hörbare Artefakte benötigen reproduzierbare Signalanalyse und geeignete Hörtests. Konkrete Fehlertoleranzen, Prozentdefinitionen und Testumgebung vorab dokumentieren, wenn sie in der Spec fehlen.

## Spec 10 – Audio Mastering

| SC | Messung und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Lautheit mit/ohne Mastering auf definiertem Material vergleichen; wahrgenommene Lautheit um 6–12 LUFS erhöhen ohne Verzerrung gemäß Spec. Messverfahren und Hörprüfung festlegen. | Mastering-Kette und Referenzmaterial. |
| 002 | Ausgangsdynamik über definierte unterschiedliche Pegelverläufe prüfen; kontrollierter Dynamikbereich ±3 dB für konsistente Ausgangspegel. | Dynamikpfad und eindeutige Messdefinition. |
| 003 | Frequenzgang über 20 Hz–20 kHz mit Testsignalen prüfen; innerhalb ±3 dB ausgewogen. Bezugskonfiguration vorab definieren. | Masteringpfad und Messaufbau. |
| 004 | Zusätzliche Verarbeitungslatenz im Live-Monitoring messen; unter 10 ms. Geräte-/Pufferlatenz separat ausweisen. | Verarbeitungskette und Zeitreferenz. |
| 005 | Presets mit dokumentiertem Referenzmaterial auf LUFS messen: Club −6, Festival −8, Radio −14. | Presets und definierte Referenzbedingungen. |
| 006 | Sichtbare Meterupdates messen: mindestens 30 FPS. | Spezifizierte Mastering-Meter. |
| 007 | Geeignete Einstellungen anhand verschiedener Signale prüfen und abhören; keine hörbaren Pump-/Verzerrungsartefakte. | Funktionierende Kette und Hörtest. |
| 008 | CPU-Verbrauch der Verarbeitung auf vorab definierter Midrange-Hardware messen; unter 10 %. | Kette, Lastprofil und Messsystem. |

## Spec 11 – DVS Control

| SC | Messung und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Timecode-Eingang bis zur entsprechenden Trackingreaktion extern messen; unter 5 ms. | Timecode-Eingang, Decoder und Zeitreferenz. |
| 002 | Referenzposition während fünf Minuten mit dekodierter Position vergleichen; innerhalb ±50 ms. | Geeignetes Timecodesignal und Positionsreferenz. |
| 003 | Bewegungen bis 10 Hz erzeugen bzw. kontrolliert ausführen; Scratch-Erkennung muss darauf reagieren. | Decoder und reproduzierbarer Bewegungsaufbau. |
| 004 | Autokalibrierung nach definiertem Startzustand prüfen; mindestens 90 % Trackinggenauigkeit innerhalb 5 Sekunden. | Kalibrierung, Referenz und Genauigkeitsdefinition. |
| 005 | Kontinuierliches Playback mindestens 30 Minuten; Synchronität ohne Drift gemäß Spec. Driftmessmethode und Auflösung festlegen. | Hardware, Decoder und Langzeitreferenz. |
| 006 | Kontrollierte Signalausfälle erzeugen; Recovery innerhalb 100 ms ohne hörbaren Aussetzer. | Fehlererkennung, Signalerzeugung und Hör-/Messaufbau. |
| 007 | Sichtbare Platter-Darstellung während Betrieb messen; 60 FPS. | UI-Integration und Frame-Messung. |
| 008 | Serato-, Traktor- und Rekordbox-Timecodes mit bekannten Referenzbewegungen prüfen; alle drei Formate korrekt decodieren. | Rechtmäßig nutzbare Testmedien, Geräte und genaue Definition der Decodiergenauigkeit. |

## Spec 12 – Stem Control

| SC | Messung und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Typische vierminütige Tracks vom Separationsstart bis zum vollständigen Ergebnis messen; innerhalb 2 Minuten. | Separationspfad, Modellrechte und Referenzhardware. |
| 002 | Vocal-Stem gegen unabhängige Referenz bewerten; mindestens 85 % Genauigkeit mit minimalem Bleed. | Referenz-Stems und fachlich definierte Prozentmetrik; SDR/SIR in dB nicht ungeprüft als Prozentwerte ausgeben. |
| 003 | Drum-Stem gegen unabhängige Referenz bewerten; mindestens 80 % Genauigkeit mit sauberer Kick und Snare. | Referenz-Stems und definierte Metrik/Hörbewertung. |
| 004 | Ladeaktion bereits zwischengespeicherter Stems bis zum nutzbaren Zustand messen; innerhalb 500 ms. | Stem-Cache und definierte Cachezustände. |
| 005 | Echtzeit-Stemumschaltung bis zum hörbaren Signalwechsel messen; innerhalb 10 ms ohne hörbare Klicks. | Synchroner Stempfad, physische Messung und Hörtest. |
| 006 | Synchronität der vier Stems über die gesamte Trackdauer gegen gemeinsame Referenz messen; innerhalb ±1 ms. | Synchronisierte Wiedergabe und Referenzmaterial. |
| 007 | Mehr als 100 Stem-Sets verwalten und Zugriff/Verdrängung testen; die 100 zuletzt genutzten Sets ohne manuelle Eingriffe vorhalten. | Cachepolitik, Speicherbudget und reproduzierbarer Bestand. |
| 008 | Batch von mindestens zehn Tracks mit unterschiedlichen Eigenschaften durchführen; keine Abstürze oder Speicherprobleme. | Batchpfad, Corpus und Ressourcenmessung. |

Prozentuale Qualitätsziele, „ohne Drift“ und andere nicht vollständig operationalisierte Formulierungen bleiben bis zur vorherigen fachlichen Definition und echten Messung offen; es werden keine Ersatzwerte behauptet.
