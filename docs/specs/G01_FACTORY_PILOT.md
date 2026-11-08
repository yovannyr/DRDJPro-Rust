# G01: Factory-Probelauf und Bestandsaufnahme

Erstellt: 2026-11-08T15:52:05.502634+00:00

HEAD: `72ecf5ecd25ea4f411588c33f3fb685c0c8b4b71`

Plan-SHA256: `92b2ae4eed4dd7eae6bfea6304464bd0736a244f7e39fc566ec2b0e046bb9490`

Matrix-SHA256: `0e99f5e44ba203a608fc7e41abbcf042a2c896073a51b6f56269a31465876b3a`

## Automatisch erhobene Befunde

- 25 Specs, 418 verschiedene specbezogene FR und 200 SC durch ID-Zählung.
- Register: 233 Einträge; Abhängigkeiten und Zyklen geprüft.
- Die frühere Analyse ist statisch; Zählung und Quellverweise ersetzen keine individuelle Laufzeitvalidierung aller Anforderungen.
- Git-Status zum Zeitpunkt der Erhebung (einschließlich bereits vorhandener Nutzeränderungen):

```text
M AGENTS.md
A  docs/specs/1-auto-updates/spec.md
A  docs/specs/10-audio-mastering/spec.md
A  docs/specs/11-dvs-control/spec.md
A  docs/specs/12-stem-control/spec.md
A  docs/specs/13-library-management/spec.md
A  docs/specs/14-recording-system/spec.md
A  docs/specs/15-live-streaming/spec.md
A  docs/specs/16-performance-pads/spec.md
A  docs/specs/17-beat-grid-sync/spec.md
A  docs/specs/18-live-remix-engine/spec.md
A  docs/specs/19-sampler-section/spec.md
A  docs/specs/2-cloud-sync/spec.md
A  docs/specs/20-multi-deck-control/spec.md
A  docs/specs/21-track-preparation/spec.md
A  docs/specs/22-neural-sync-engine/spec.md
A  docs/specs/23-mix-history-analytics/spec.md
A  docs/specs/24-smart-playlist-manager/spec.md
A  docs/specs/25-enhanced-harmonic-mixing/spec.md
A  docs/specs/3-bpm-detection/spec.md
A  docs/specs/4-midi-controller/spec.md
A  docs/specs/5-streaming-integration/spec.md
A  docs/specs/6-key-detection/spec.md
A  docs/specs/7-advanced-waveform/spec.md
A  docs/specs/8-video-mixing/spec.md
A  docs/specs/9-ai-automix/spec.md
AM docs/specs/MISSING_FEATURES_ANALYSIS.md
?? build_output.txt
?? docs/specs/FACTORY_SLICES.json
?? docs/specs/IMPLEMENTATION_PLAN.md
?? docs/specs/SPEC_REQUIREMENTS_MATRIX.md
?? test_output.txt
?? tools/
```

## ADR-Entwurf: native Umsetzung

Bestehendes Rust-/Slint-Programm und explizite Konstruktion bleiben Ausgangspunkt. Browser-/Electron-Begriffe werden pro Spec auf native Plattformdienste abgebildet. Sichtbares Verhalten und Messziele bleiben Anforderungen; konkrete Bibliotheken und Plattformunterstützung sind vor ihren jeweiligen Slices zu evaluieren. Der Entwurf ist keine Freigabe neuer Abhängigkeiten.

## Konflikt- und Entscheidungsregister

| Thema | Sachstand | Status |
|---|---|---|
| Camelot (Specs 6/25) | Wheel-Nachbarschaft und Halbtonbeispiele fachlich prüfen. | Offen: musikalische Regel und Entscheidung dokumentieren. |
| Cloud (Spec 2) | Last-write-wins und Ziel für Änderungserhalt im Mehrgerätebetrieb zusammen bewerten. | Offen: Konflikt- und Messverfahren. |
| Video-/UI-FPS (7/8/15) | Unterschiedliche Kennzahlen und Lastbedingungen unterscheiden. | Offen: Testumgebungen und Messmethodik. |
| Analysezeiten (3/21) | Gemeinsame Messbedingungen, Cache und Datensatz festlegen. | Offen. |
| Übergänge/Retention (23) | Vollständige Ereigniserfassung und konfigurierbare Begrenzung abgrenzen. | Offen. |
| Musikdienste, DVS, Modelle, Encoder | Zugänge, Lizenzen und tatsächliche technische Verfügbarkeit prüfen. | Offen; eigenes Gate G02. |

## Mess- und Bestandsplan

Für jeden FR/SC sind Quelle, vorhandene Implementierung, Test, Plattform und Nachweis getrennt zu erfassen. Ein annotiertes, lizenziertes Musik-Corpus; Hardware-/OS-/Audiointerface-Matrix; CPU-/Latenz-/Dropout-Verfahren; sowie Kohorten und Beobachtungszeiträume für Nutzer- und Betriebsziele sind noch festzulegen. Bestehende Tests, synthetische Signale und MC4000-Berichte sind keine neue Hardware- oder Hörabnahme.

**G01-Status: offen.** Dies ist ein Probelauf und ein erster überprüfbarer Entwurf. Die tatsächliche Einzelprüfung aller 618 FR/SC und die Entscheidungstore D1–D3/D5/D6 sind nicht abgeschlossen. Es werden keine abhängigen Slices freigegeben.

## Prüfergebnis des Probelaufs

Das Register umfasst 83 Pakete und 150 zugeordnete Umsetzungsslices; die 25 Abnahmepakete enthalten ihre 200 SC. `factory.py validate` und `factory.py check G01` waren nach Korrektur der Windows-Pfadnormalisierung erfolgreich; acht Factory-Tests bestanden. Ein früher Testlauf hatte den Pfadfehler gefunden und ist kein Erfolgsnachweis. Status nach dem Probelauf: **in Arbeit**, keine Gesamtfreigabe. QWEN und DeepSeek erreichten bei der Beauftragung das Zeitlimit; die Umsetzung erfolgte mit dem vom Nutzer benannten Fallback GPT-6.1 Terra.
