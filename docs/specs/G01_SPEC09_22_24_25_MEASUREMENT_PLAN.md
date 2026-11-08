# G01 – Messplan für Empfehlungen und intelligente Übergänge

Stand 08.11.2026. Grundlagen: [Spec 9](9-ai-automix/spec.md), [Spec 22](22-neural-sync-engine/spec.md), [Spec 24](24-smart-playlist-manager/spec.md), [Spec 25](25-enhanced-harmonic-mixing/spec.md), [G01-Bestandsabgleich](G01_ADVANCED_FEATURES_AUDIT.md) und ein Entwurf von `deepseek-v4-flash-sovereign`. Modellvorschläge wurden gegen die Originaltexte geprüft. **Keine dieser 32 Messungen ist ausgeführt oder bestanden.**

Die vollständigen End-to-End-Funktionen sind noch nicht nachgewiesen. Vor Messungen Commit, Plattform, Release-Build, Datensatz/Lizenzen, Cache, Testbedingungen, Rohwerte, Stichprobe und Ausschlüsse festlegen. Für Musikqualität sind unabhängig annotierte Referenzen und dokumentierte DJ-Hör- bzw. Nutzertests nötig. Ein vom System selbst berechneter Score ist kein unabhängiger Beweis für Hörqualität. Subjektive Begriffe und Prozent-Nenner vorab definieren, ohne einen zusätzlichen Grenzwert als Spec-Vorgabe auszugeben.

## Spec 9 – AI Auto-Mix

| SC | Messung und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Für definierte Trackpaare ausgegebene Kompatibilitätsscores prüfen; vorgeschlagene Tracks mit mindestens 85 % Score. | Scoring und Testdaten. |
| 002 | Übergänge unter dokumentierten Bedingungen blind abhören; laut Spec nahtlos ohne hörbares Clashing oder Key-Dissonanz. | Übergangsengine und Hörtest. |
| 003 | Generierung von Start bis Plan bei Bibliotheken unter 1000 Tracks messen; innerhalb 5 Sekunden. | Planer und definierte Hardware. |
| 004 | Geplante und tatsächliche Energieverläufe gegen den konfigurierten Modus prüfen; mindestens 90 % der Übergänge folgen diesem. | Energie-Referenz und messbare Modusdefinition. |
| 005 | Tempoverlauf während Übergängen erfassen; gleichmäßiger BPM-Übergang innerhalb der eingestellten Dauer. | Sync-/Übergangspfad und Definition von gleichmäßig. |
| 006 | An verschiedenen Zeitpunkten manuell übernehmen; Wiedergabe darf nicht unterbrochen werden. | Sicherer Override und Audio-Messung. |
| 007 | Scoring-Genauigkeit vor und nach zehn manuellen Overrides gegen unabhängige Referenz vergleichen; Verbesserung um 10 %. | Lernfunktion und vorab definierte Genauigkeitsbasis. |
| 008 | DJ-Test mit vorab definierten Vorschlägen; mindestens 80 % finden sie ohne manuelle Änderung akzeptabel. | Nutzergruppe und dokumentiertes Testdesign. |

## Spec 22 – Neural Sync

| SC | Messung und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Analyse typischer vierminütiger Tracks messen; innerhalb 2 Sekunden. | Phrasenanalyse, Testdaten und definierte Hardware. |
| 002 | Erkannte Haupt-**Phrasengrenzen** gegen unabhängige manuelle Annotation vergleichen; 85 % Genauigkeit. Zuordnungstoleranz und Nenner vorher definieren. | Annotiertes Corpus und Phrasendetektor. |
| 003 | Nach Anwendung eines Vorschlags tatsächlichen Beat-Timingfehler erfassen; unter 10 ms. | Vorschlagspfad und Audio-Messaufbau. |
| 004 | Für kompatible Trackpaare die drei höchstbewerteten Vorschläge prüfen; jeweils über 70 % Confidence. Score zusätzlich gegen Referenz validieren. | Vorschläge und definierte Confidence. |
| 005 | Nach Analyse beider Tracks bis Vorschlagsgenerierung messen; innerhalb 500 ms. | Vorschlagsengine und Hardware. |
| 006 | Sichtbare Wellenform während Wiedergabe erfassen; 30 FPS ohne gedroppte Frames. | Phrasenansicht und Frame-Messung. |
| 007 | Energiekompatibilitätsfilter gegen unabhängige Bewertungen vergleichen; 90 % Relevanz. | Referenzdefinition und bewertetes Corpus. |
| 008 | DJ-Nutzung nach initialer Lernphase erheben; mindestens 50 % ihrer Übergänge nutzen Neural Sync. | Nutzertest, Lernphase und definierter Nenner. |

## Spec 24 – Smart Playlist Manager

| SC | Messung und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Generierung bei 1000 Tracks von Auftrag bis fertiger Liste messen; innerhalb 3 Sekunden. | Generator und definierte Hardware. |
| 002 | Nutzerfeedback für Empfehlungen auswerten; mindestens 80 % Relevanz. | Feedbackpfad und repräsentative Stichprobe. |
| 003 | Übergangsscores erzeugter Playlists prüfen; mindestens 85 % der Übergänge mit Qualitätswert ≥70. | Playlistgenerator und dokumentierter Scorevertrag. |
| 004 | Energieplaylists gegen vorher definierte Muster auswerten; 95 % Übereinstimmung. | Energieanalyse und Referenz für Muster. |
| 005 | Harmonische Playlists anhand korrigierter Camelot-Regeln prüfen; mindestens 90 % kompatible Übergänge. | Regelmatrix und Generator. |
| 006 | Gespeicherte Playlistmetadaten über definierte Stichprobe messen; durchschnittlich unter 5 KB je Playlist. | Persistenzschema und Speicherzählung. |
| 007 | Regeneration bestehender Listen messen; innerhalb 2 Sekunden. | Regenerationspfad und Hardware. |
| 008 | Innerhalb des ersten Monats erheben, ob DJs durchschnittlich mindestens drei Smart Playlists erstellen und verwenden. | Nutzerkohorte und Beobachtungszeitraum. |

## Spec 25 – Enhanced Harmonic Mixing

| SC | Messung und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Nutzertest zur Position des aktuellen Keys auf dem Wheel; 95 % der Testpersonen erkennen sie innerhalb 5 Sekunden. | Wheel, Stichprobe und Testaufbau. |
| 002 | DJ-Hörbewertung empfohlener Übergänge; mindestens 90 % harmonisch glatt. | Empfehlungen, verblindete Bewertung und Stichprobe. |
| 003 | Trackfinder in Bibliotheken bis 5000 Tracks messen; Ergebnisse innerhalb 300 ms. | Finder, Testbibliothek und Hardware. |
| 004 | Key-Zählung mit unabhängiger Datenbankreferenz für alle 24 Keys vergleichen; 100 % Präzision ohne Zählfehler. | Analyzer und Referenzdatensatz. |
| 005 | Neulinge nach zehn Minuten anhand vorher definierter Aufgaben prüfen; 80 % verstehen das Wheel-Konzept. | Nutzertest und operationalisierte Verständniskriterien. |
| 006 | Klicks/Hover bis sichtbare Reaktion messen; innerhalb 50 ms ohne wahrgenommenen Lag. | Interaktives Wheel und sichtbare Frame-Messung. |
| 007 | DJ-Selbsteinschätzung der Übergangsglätte nach einem Monat mit Ausgangswert vergleichen; berichtete Steigerung um 40 %. | Kohorte, Ausgangswert und Fragebogendesign. |
| 008 | Erste Woche beobachten; 60 % aktivieren und nutzen Harmonic Mixing regelmäßig. | Kohorte und vorab definierte Regelmäßigkeit. |

Alle 32 SC bleiben offen. Wo die Spec keine Stichprobengröße, Toleranz oder Bewertungsdefinition angibt, ist diese vor der Messung zu vereinbaren; eine selbstgesetzte Ersatzgrenze ist kein Originalkriterium.
