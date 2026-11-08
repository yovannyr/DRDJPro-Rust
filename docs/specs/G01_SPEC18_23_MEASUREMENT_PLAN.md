# G01 – Messplan für Live-Remix und Mix-History

Stand 08.11.2026. Grundlagen: [Spec 18](18-live-remix-engine/spec.md), [Spec 23](23-mix-history-analytics/spec.md), [G01-Ersteinstufung](G01_EXTERNAL_FEATURES_AUDIT.md), [History-Abgleich](G01_ADVANCED_FEATURES_AUDIT.md) und ein Entwurf von `deepseek-v4-flash-sovereign`. Alle 16 Messungen sind **geplant und offen**. Modellvorschläge zu 100 % CPU-Last oder WAV-/MP3-Sessionexport sind keine zusätzlichen Spec-Vorgaben.

Für jeden Lauf Commit, Release-Build, Hardware, OS, Audiointerface, Puffer, Last, Testdaten, Zeitreferenz und Rohwerte festhalten. Die Aufzeichnung von Slice- oder Übergangsereignissen, visuelles Replay und Audioaufnahme sind eigenständige Funktionen. Subjektive Begriffe, Stichproben und Vergleichsbasis vorab konkretisieren.

## Spec 18 – Live Remix Engine

| SC | Messung und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Auto-Slicing eines typischen vierminütigen Tracks von Auftrag bis fertigem Slice-Set: innerhalb 5 Sekunden. | Slicing-Engine, Testtrack und definierte Hardware. |
| 002 | Übergänge nacheinander abgespielter Slices mit Referenzsignal analysieren: samplegenau und unter 1 ms Lücke gemäß Spec. | Audioausgabe, Slice-Engine und Messaufbau. |
| 003 | Pattern-Ladeaktion bis wiederhergestelltes Arrangement messen: innerhalb 500 ms. Patternumfang und Cachezustand ausweisen. | Pattern-Persistenz. |
| 004 | Tatsächliche Live-Trigger gegen erfasste Performance-Zeitstempel vergleichen: Präzision unter 10 ms. | Ereignisaufzeichnung und gemeinsame Referenzzeit. |
| 005 | Arrangement während UI-Wiedergabe bei 60 FPS abspielen und abhören; keine hörbaren Glitches. Last und Audio-Dropouts dokumentieren. | Wiedergabepfad, sichtbare Frame- und Audiomessung. |
| 006 | Zusätzliche Latenz durch Slice-Effekte messen; höchstens 20 ms gemäß Spec. Effekttyp und Referenz ohne Effekt angeben. | Slice-FX und physischer Zeitaufbau. |
| 007 | Preset laden bis vollständiges Setup wiederhergestellt ist: innerhalb 2 Sekunden. | Preset-Persistenz und festgelegte Komplexität. |
| 008 | Mindestens 100 Slices je Track bearbeiten/abspielen; Performance-Degradation gegenüber vorab festgelegter Referenz prüfen. | Engine, Testdaten und Degradationsdefinition. |

## Spec 23 – Mix History & Analytics

| SC | Messung und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Tatsächliche Übergänge während Normalbetrieb gegen persistierte Ereignisse vergleichen; 100 % erfasst, kein Datenverlust. Recovery-Fälle separat berichten. | Session-/Transition-Eventmodell und unabhängige Referenz. |
| 002 | Tatsächlichen Datenzuwachs über Sessions und Übergänge messen; durchschnittlich unter 1 KB pro Transition. Speicherformat und Zählweise ausweisen. | Persistenzmodell und repräsentative Sessions. |
| 003 | CPU mit/ohne aktiver Mix-History bei identischer Last vergleichen; unter 5 % Overhead. Audio-Dropouts zusätzlich erfassen. | Eventpfad, Hardware und Lastprofil. |
| 004 | Liste mit mindestens 100 Sessions von Anfrage bis nutzbare Anzeige messen; innerhalb 200 ms. | Sessiondatenbank und UI. |
| 005 | Automatischen Qualitätsscore mit DJ-Selbstbewertung vergleichen; 80 % Übereinstimmung. Skala, Auswertung und Stichprobe vorab definieren. | Scores, DJ-Panel und Testprotokoll. |
| 006 | Export einer typischen Session mit 30 Übergängen von Aktion bis fertiger Datei messen; innerhalb 1 Sekunde. Spezifizierte Datenexportformate getrennt prüfen. | Exportfunktion, Zielmedium und Sessiondaten. |
| 007 | Vollständigkeit und Lesbarkeit über mindestens sechs Monate einschließlich Migrationen und Wiederherstellung prüfen; keine Degradation. | Dauerlauf oder entsprechend dokumentierte Langzeitbeobachtung; Zeitsimulation gesondert kennzeichnen. |
| 008 | Bei einer vorab beschriebenen DJ-Kohorte die wöchentliche History-Nutzung erfassen; mindestens einmal pro Woche im Durchschnitt gemäß Spec. | Nutzergruppe, Beobachtungszeitraum und Definition von „review“; Datenschutz beachten. |

Die vorhandene trackbezogene Historie ersetzt keine Sessionereignisse oder Audioaufnahme. Alle Kriterien bleiben bis zu belegten Messungen offen.
