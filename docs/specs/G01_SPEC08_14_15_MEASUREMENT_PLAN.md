# G01 – Messplan für Video, Aufnahme und Livestream

Stand 08.11.2026. Grundlagen: [Spec 8](8-video-mixing/spec.md), [Spec 14](14-recording-system/spec.md), [Spec 15](15-live-streaming/spec.md), [G01-Ersteinstufung](G01_EXTERNAL_FEATURES_AUDIT.md) und ein Entwurf von `deepseek-v4-flash-sovereign`. Der Entwurf wurde an den Originaltext angepasst: Aufnahme-Splits richten sich nach konfigurierbaren Zeitintervallen, nicht nach einer erfundenen Dateigröße; zusätzliche Teilnehmerzahlen und Testdauern sind keine Spec-Grenzwerte. **Alle 24 Messungen sind offen.**

Je Lauf Commit, Release-Paket, Betriebssystem, CPU/GPU, verwendete Decoder/Encoder, Videoquelle, Audiointerface, Samplerate, Buffer, Netzwerk, Zeitreferenz und Rohdaten erfassen. Tatsächliche Ein-/Ausgabe, Dateiinhalte und Empfang getrennt von internen Zeitstempeln messen. Anbieterzugänge und Rechte müssen vor realen Tests geklärt werden; eine Testbeschreibung behauptet keine aktuelle Verfügbarkeit.

## Spec 8 – Video Mixing

| SC | Messung und Originalziel | Voraussetzung / Status |
|---|---|---|
| 001 | Dateiauswahl bis zum ersten sichtbaren Frame: innerhalb 2 Sekunden, unterschiedliche unterstützte Formate prüfen. | Video-Laden und reproduzierbare Dateien fehlen als vollständiger Nachweis. |
| 002 | Sichtbare Wiedergabe und Framefolge auf typischer Hardware messen: mindestens 30 FPS ohne gedroppte Frames. | Videoausgabe und Messaufbau. |
| 003 | Crossfader über den Bereich bewegen; Ausgabe auf Tearing und Stottern visuell und per Framefolge prüfen. | Zwei Videoquellen und Compositing. |
| 004 | Effektänderung bis zur sichtbaren Ausgabe messen: unter 16 ms als 60-FPS-Ziel. | Effektpfad und sichtbare Zeitmessung. |
| 005 | Keying gegen annotierte Farbflächen und Kantenreferenz prüfen: mindestens 95 % der Zielfarbe entfernt, saubere Kanten bei moderater Toleranz. | Chroma-Key und definierte Referenzbilder. |
| 006 | Übergangsbeginn bis Ende gegen gewählte Dauer: ±100 ms. Verschiedene Typen und Dauern prüfen. | Übergangspipeline. |
| 007 | Layoutwechsel bis sichtbares neues Layout: innerhalb 200 ms ohne schwarze Frames. | Layouts und Frameaufzeichnung. |
| 008 | 1080p-Video bei 30 FPS auf Midrange-Hardware mit Vierkern-CPU und integrierter GPU gemäß Spec betreiben; Last und Framefolge ausweisen. | Definiertes Zielgerät und Videoausgabe. |

## Spec 14 – Recording System

| SC | Messung und Originalziel | Voraussetzung / Status |
|---|---|---|
| 001 | Audioausgabe bzw. festgelegten Abgriff bis zu den aufgenommenen Daten messen: unter 100 ms. Schreib-/Pufferzeit und Audiosignal getrennt erfassen. | Aufnahme- und Schreibpfad. |
| 002 | Angezeigte geschätzte Dateigröße mit tatsächlicher Aufnahmegröße vergleichen: innerhalb ±5 %. Zeitpunkte und Formate dokumentieren. | Größenanzeige und Encoder. |
| 003 | WAV-Aufnahme decodieren und Signal/Format gegen Referenz prüfen: volle 1411-kbit/s-/44,1-kHz-Treue. Testformat und Kanäle dokumentieren. | WAV-Encoder und Referenzsignal. |
| 004 | MP3 320 kbps gegen verlustfreies Referenzsignal in verblindetem Hörtest vergleichen: gemäß Spec nicht unterscheidbar. Methodik, Teilnehmer und Statistik vorab festlegen. | MP3-Encoder und Hörtest; keine erfundene Teilnehmer-Mindestzahl. |
| 005 | Zeitgesteuerten Auto-Split an konfigurierten Intervallen auslösen; decodierte Dateigrenzen vergleichen: Lücke unter 10 ms. | Zeitbasierter Split und Referenzsignal. |
| 006 | Aufnahme pausieren und fortsetzen; resultierende Audiozeit gegen definierte aktive Aufnahmezeit vergleichen: Drift unter 50 ms. | Pause-/Resume-Pfad und Referenzzeit. |
| 007 | Mindestens 1000 Aufnahmen anzeigen und sortieren; Reaktionsverhalten und UI-Stalls erfassen, Ziel laut Spec ohne Lag. | Aufnahmekatalog und vorab definierte Lag-Messung. |
| 008 | Nach Aufnahmeende bis zur aktualisierten Speicherverbrauchsanzeige messen: innerhalb 1 Sekunde. | Persistenter Katalog und Anzeige. |

## Spec 15 – Live Streaming

| SC | Messung und Originalziel | Voraussetzung / Status |
|---|---|---|
| 001 | Go-Live-Aktion bis bestätigter Streamstart beim definierten Empfänger: innerhalb 10 Sekunden. | Berechtigter Teststream und Empfänger. |
| 002 | Änderung der tatsächlichen Zuschauerzahl bis sichtbare Aktualisierung: innerhalb 10 Sekunden. | Kontrollierte Testclients und Plattformzugang. |
| 003 | Zeitabstand sichtbarer Bitrate-/FPS-/Drop-/Verbindungsstatistik: Aktualisierung jede Sekunde. | Statistikpfad und Zeitmessung. |
| 004 | Externe Chatnachricht absenden bis zur Anzeige in der App: innerhalb 2 Sekunden. | Unterstützter Chat und Testkonto. |
| 005 | Bei stabiler Verbindung Zielbitrate und tatsächlich gesendete Bitrate erfassen: mindestens 95 % der Zielbitrate. | Reproduzierbare Verbindung und Netzmetrik. |
| 006 | Bei ausreichender Bandbreite gesendete und verlorene Frames zählen: unter 1 % gedroppte Frames. | Framezähler und definierte Verbindung. |
| 007 | Temporäre Trennung auslösen und automatische Wiederverbindung messen: innerhalb 30 Sekunden erfolgreich. | Kontrollierter Netzausfall und Reconnectpfad. |
| 008 | Overlays während aktivem Stream darstellen; 60 FPS ohne Performanceeinfluss gemäß Spec. Vergleich mit/ohne Overlay sowie Messdefinition für Einfluss vorab dokumentieren. | Overlay-Renderer und definierter Vergleich. |

Originalanforderungen, Fehlerfälle und Anbieterrechte bleiben maßgeblich. Keine der Tabellenzeilen stellt ein gemessenes Ergebnis dar.
