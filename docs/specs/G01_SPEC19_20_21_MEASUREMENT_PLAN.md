# G01 – Messplan für Sampler, Mixer und Track Preparation

Stand 08.11.2026. Grundlagen: [Spec 19](19-sampler-section/spec.md), [Spec 20](20-multi-deck-control/spec.md), [Spec 21](21-track-preparation/spec.md), zugehörige G01-Einzelabgleiche und ein Entwurf von `deepseek-v4-flash-sovereign`. Der Entwurf wurde gegen den Code und die Specs korrigiert: Vier Samplerstimmen und zwei Decks existieren; Samplebibliothek, Vierdeckmodus, vollständige Mute-/Solo-Funktionen und Master-Limiter sind nicht als Gesamtfunktion nachgewiesen. **Alle nachstehenden Messungen sind offen.**

Pro Lauf Commit, Release-Build, Testdaten, Betriebssystem, Rechner, Audiointerface, Treiber, Sample-Rate, Buffer und Eingabegerät erfassen. Physische Audio-, Eingabe- und sichtbare UI-Reaktionen mit einem gemeinsamen Referenzaufbau messen; interne Zeitstempel und Headless-Tests nur ergänzend nutzen. Fehlversuche, Dropouts, Rohwerte und den Nenner berichten; Wiederholungszahl und unklare Begriffe vor der Abnahme festlegen.

## Spec 19 – Sampler

| SC | Messung und Originalziel | Voraussetzung / Status |
|---|---|---|
| 001 | Physischer Trigger bis Audioausgang: unter 10 ms. | Hardwaremessung fehlt. |
| 002 | Öffnen der Samplebibliothek bis zur sichtbaren Darstellung: innerhalb 500 ms. | Vollständiger Samplebibliotheks-Browser nicht nachgewiesen. |
| 003 | Änderung des Pegels bis zur Auswirkung auf das ausgegebene Audiosignal: innerhalb 50 ms. | Einzelpegel je Pad und Messung nicht vollständig belegt. |
| 004 | Stop-All-Eingabe bis alle Sampleausgänge still sind: innerhalb 10 ms; sämtliche aktiven Slots prüfen. | Funktion existiert für vier Slots; Hardwaremessung fehlt. |
| 005 | Acht Samples gleichzeitig über eine definierte Dauer abspielen; keine Audio-Dropouts. | Bestehender Sampler besitzt vier statt acht Stimmen. |
| 006 | Auswahl einer unterstützten Audiodatei bis zum spielbereiten Pad: innerhalb einer Sekunde. Formate und Datei-/Cachegrößen dokumentieren. | Reproduzierbarer Messlauf fehlt. |
| 007 | Trigger bis sichtbar aktualisierter Padanimation: innerhalb 16 ms bei 60 FPS. | Vollständige Animation und sichtbare Frame-Messung nicht nachgewiesen. |
| 008 | Bibliothek mit mindestens 500 Samples durchsuchen; Suchreaktion und UI-Stalls dokumentieren. Ziel: kein Such-Lag. | Suchbare Samplebibliothek fehlt als vollständiger Nachweis; Lag-Interpretation vorab festlegen. |

## Spec 20 – Multi-Deck Control

| SC | Messung und Originalziel | Voraussetzung / Status |
|---|---|---|
| 001 | Umschaltaktion zwischen Zwei- und Vierdeckmodus bis zum wirksamen Zustand: innerhalb 200 ms. | Vierdeckmodus nicht nachgewiesen. |
| 002 | Faderbewegung bis zur Auswirkung auf das ausgegebene Audiosignal: innerhalb 10 ms. | Physischer Messaufbau und Vierdeckumfang fehlen als Nachweis. |
| 003 | EQ-Änderung bis zur Auswirkung auf das Audiosignal: innerhalb 20 ms; alle Bänder/Kanäle prüfen. | Zweideck-EQ vorhanden; vollständige Messung fehlt. |
| 004 | Mute-/Solo-Eingabe bis zum entsprechenden Audiostatus: innerhalb 10 ms. | Vollständige Mute-/Solo-Funktion nicht nachgewiesen. |
| 005 | Sichtbare Aktualisierung aller Kanalmeter bei unterschiedlicher Last messen: mindestens 30 FPS. | Vierdeckmeter und sichtbare Frameratenmessung fehlen als Nachweis. |
| 006 | Vier Decks gleichzeitig auf vorab definierter Midrange-Hardware betreiben; CPU-Nutzung unter 30 %, Messdauer, Last und Dropouts ausweisen. | Vierdeckmodus und verbindliches Referenzsystem fehlen. |
| 007 | Master mit definierten Überpegelsignalen speisen; Limiter muss Clipping verhindern, Attack unter 1 ms. Sample-Peak und True-Peak-Methode getrennt dokumentieren. | Master-Limiter ist nicht als Spec-Funktion nachgewiesen. |
| 008 | Crossfader über den gesamten Bereich in beiden Richtungen bewegen; Verlauf messen und abhören. Ziel: keine Sprünge oder Stufen. | Aufbau, Stimuli und Hörprüfung fehlen. |

## Spec 21 – Track Preparation

| SC | Messung und Originalziel | Voraussetzung / Status |
|---|---|---|
| 001 | Typische vierminütige Tracks vom Analyseauftrag bis zum verfügbaren Ergebnis messen: innerhalb 10 Sekunden; gemäß dokumentierter Nutzerentscheidung mit Spec 3 abgestimmt. | Corpus, Zielhardware und Messlauf fehlen. |
| 002 | Unabhängig BPM-annotiertes Electronic-/Dance-Corpus auswerten: mindestens 95 % Genauigkeit. Die konkrete Fehlertoleranz und Behandlung von Oktavfehlern vorab dokumentieren. | Annotiertes Corpus und vollständige Auswertung fehlen. |
| 003 | Unabhängig annotiertes Musik-Corpus für Key und ausgegebene Confidence auswerten: mindestens 85 % Key-Genauigkeit mit Confidence-Werten. | Confidence-Ausgabe und Corpus sind nicht vollständig belegt. |
| 004 | Slideränderung bis wirksame Grid-Anpassung messen: innerhalb 50 ms. | Grid-Editor vorhanden; Spec-Slider/Messung nicht vollständig belegt. |
| 005 | Cue-Erstellung und Sprung jeweils von Eingabe bis wirksame Position bzw. Audioausgabe messen: unter 100 ms. | Cue-Pfade vorhanden; physische Messung fehlt. |
| 006 | Wellenform samt Cue-/Beat-Overlays während Wiedergabe sichtbar messen: mindestens 30 FPS. | Dokumentierter Messlauf fehlt; automatisch erkannte Beatmarker nicht vollständig belegt. |
| 007 | Mindestens 20 Tracks im Batch analysieren, während definierte UI-Interaktionen fortlaufend erfolgen; kein UI-Freeze. | Batchpfad vorhanden; Lastlauf und Freeze-Definition fehlen. |
| 008 | Analyseergebnis speichern und nach erneutem Laden verfügbar machen: innerhalb 200 ms; Speicher- und Ladezeit getrennt ausweisen. | Dokumentierter Roundtrip und Messung fehlen. |

Die Originalkriterien und ihre Unterpunkte bleiben maßgeblich. Vorschläge zu Testdauer und Stichprobe werden vorab konkretisiert; weder einzelne Zeitstempel noch vorhandene Tests belegen die Gesamtakzeptanz.
