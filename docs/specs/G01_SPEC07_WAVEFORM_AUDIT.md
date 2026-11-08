# G01 – Einzelabgleich Spec 7: Advanced Waveform

Quellstand `df4966f`, statischer Abgleich am 08.11.2026. [Spec 7](7-advanced-waveform/spec.md) und [Register](SPEC_REQUIREMENTS_MATRIX.md) enthalten den vollständigen Wortlaut. Der Workspace-Test auf `b783a98` bestand mit 504 Tests und sechs ignorierten Tests; nachfolgende Commits änderten Dokumente. Die UI wurde in diesem Abgleich nicht visuell oder unter Last abgenommen.

| ID | Status | Nachweis und verbleibende Arbeit |
|---|---|---|
| FR-001 | Teilweise | `DetailPeaks` in [waveform.rs](../../src/waveform.rs) berechnet Maximalwerte; ein zusätzlich angezeigter RMS-Wellenformwert ist nicht nachgewiesen. |
| FR-002 | Offen | Ein STFT-Spektrogramm ist im untersuchten Wellenformpfad nicht nachgewiesen. Key-Analyse mit FFT ist kein Spektrogramm. |
| FR-003 | Offen | [waveform_color.rs](../../src/waveform_color.rs) dokumentiert drei approximative Bänder; gefordert sind sieben explizite Bänder. |
| FR-004 | Teilweise | `BandEnergy::color` färbt nach relativer Tief-/Mittel-/Hochenergie; Genauigkeit und die spezifizierten Bänder bleiben offen. |
| FR-005 | Offen | Die acht auswählbaren Farbschemata sind nicht als UI-Vertrag nachgewiesen. |
| FR-006 | Teilweise | Gridmarker im Wellenformpfad vorhanden; das Grid ist manuell und keine automatische Beatdetektion. |
| FR-007 | Teilweise | Taktlinien/Downbeats aus [beatgrid.rs](../../src/beatgrid.rs) vorhanden; spezifizierte Phrasenmarker und alle Taktfälle prüfen. |
| FR-008 | Offen | `OVERVIEW_BINS` ist 256; frei konfigurierbare Auflösung 256–2048 ist nicht nachgewiesen. |
| FR-009 | Offen | Spektrogramm mit vier FFT-Größen ist nicht nachgewiesen. |
| FR-010 | Offen | Frequenzachsen eines Spektrogramms sind nicht nachgewiesen. |
| FR-011 | Offen | Spektrogramm-Normalisierung auf −90 bis 0 dB ist nicht nachgewiesen. |
| FR-012 | Offen | Interaktives Zeit-/Frequenz-Fadenkreuz ist nicht nachgewiesen. |
| FR-013 | Offen | Live-Frequenzspektrum während der Wiedergabe ist nicht als Spec-Funktion nachgewiesen. |
| FR-014 | Teilweise | Overview und positionsbezogene Detailansicht sind vorhanden; separater Live-Spektrummodus ist nicht belegt. |
| FR-015 | Teilweise | `DetailPeaks::range` folgt der Position; visuelle Framerate und Synchronität sind separat zu prüfen. |
| FR-016 | Teilweise | Wellenform-/Positionsnavigation in [app.slint](../../ui/app.slint) und [controller.rs](../../src/controller.rs); Klickgenauigkeit und alle Ansichten testen. |
| FR-017 | Offen | Zeit- und Energie-Tooltips beim Hover sind nicht vollständig nachgewiesen. |
| FR-018 | Offen | PNG-Export von Wellenform und Spektrogramm ist nicht nachgewiesen. |

SC-001–008 bleiben offen: Lade-/Renderzeit, 60-FPS-Playhead, Klickgenauigkeit, Farbklassifizierung, Grid-Ausrichtung und Hover-/Moduswechsel benötigen die spezifizierten Daten und Messungen. Drei Filterbänder sind eine Darstellungsheuristik und kein Beleg für sieben STFT-Bänder.

Der [gemeinsame Messplan](G01_SPEC07_16_MEASUREMENT_PLAN.md) beschreibt alle acht Wellenform- und acht Padmessungen.
