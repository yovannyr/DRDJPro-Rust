# G01 – Entscheidungen und Messkonzept

Stand 08.11.2026, Quellstand `9bd35f45452934297194f5bd00c77f4c9937bb9e`. Diese Festlegungen konkretisieren den Plan; sie belegen noch keine Implementierung oder erreichten Erfolgskriterien. Wenn eine Originalanforderung nachweislich nicht gleichzeitig erfüllbar ist, bleibt die betroffene Anforderung bis zur abgestimmten Klärung offen; kein Grenzwert wird stillschweigend abgesenkt.

## D1: Harmonische Beziehungen

**Entscheidung für die spätere Implementierung:** Tonart intern als Tonika modulo zwölf und Dur/Moll darstellen. Camelot-Zahl und tatsächliches chromatisches Intervall getrennt berechnen. Gleiche Tonart, Wheel-Nachbar ±1 und paralleles Dur/Moll gemäß Spec 6 als `Perfect` behandeln. Energierichtung aus dem Tonhöhenintervall bestimmen, nicht aus der Differenz der Camelot-Zahlen. Ein Paar kann mehrere Eigenschaften besitzen; die Darstellung der Übergangsklasse braucht eine dokumentierte Priorität, ohne andere Eigenschaften zu löschen.

**Konkreter Widerspruch:** Die vorhandene [Camelot-Zuordnung](../../src/audio_analyzer.rs) ordnet C-Dur `8B`, G-Dur `9B` und Cis-Dur `3B` zu. `8B → 9B` entspricht damit sieben Halbtönen aufwärts bzw. fünf abwärts, nicht einem Halbton; die Beispiele in [Spec 25](25-enhanced-harmonic-mixing/spec.md) FR-025-003 sind mit der eigenen Definition nicht gleichzeitig richtig. Testfälle: `8B → 9B` Wheel-Nachbar; `8B → 3B` ein Halbton aufwärts; zyklischer Übergang `12 → 1`; Dur/Moll bei gleicher Zahl; alle 24×24 Paare. **Offen:** Die explizite Korrektur der widersprüchlichen Sollbeispiele und die endgültige Einordnung der übrigen Schwierigkeitsklassen brauchen fachliche Entscheidung. Bis dahin FR-025-003/004 und abgeleitete Akzeptanz nicht als erfüllt markieren.

## D2: Cloud-Konflikte

**Entscheidung für die spätere Implementierung:** unterschiedliche Objekte getrennt behandeln; unabhängige Ergänzungen erhalten. Gleichzeitig geänderte Werte desselben Objekts mit deterministisch geordneten Änderungsrevisionen nach Last-write-wins behandeln. Eine konkurrierende Löschung erhält gemäß User Story der [Spec 2](2-cloud-sync/spec.md) Vorrang vor Änderung; dafür versionierte Löschmarker vorsehen. Wiederholter Upload und Neuversuch müssen idempotent sein. Keine Geheimnisse oder Authentifizierungsdaten synchronisieren, bevor Datenklassifikation und Isolationskonzept feststehen.

**Testfälle:** disjunkte Ergänzungen, dieselbe Eigenschaft gleichzeitig ändern, Ändern gegen gleichzeitiges Löschen, erneute Zustellung, abgestürzte Synchronisation und Geräte mit Zeitabweichung. Ein serverseitig eindeutiges Ordnungsmerkmal oder ein anderer deterministischer Konfliktvertrag ist nötig; lokale Geräteuhren allein sind dafür nicht hinreichend. Die genaue Behandlung nicht-konkurrierender späterer Wiederherstellung und die sichere Aufbewahrungsdauer der Löschmarker bleiben offen.

**Grenze:** FR-012 fordert LWW, die User Story gibt bei konkurrierender Löschung eine spezifische Ausnahme vor; FR-013 fordert Offline-Warteschlange, obwohl die Out-of-Scope-Liste eine vollständige Offline-first-Architektur ausschließt. Eine persistente Warteschlange für lokale Änderungen ist daher einzuplanen; daraus wird keine zusätzliche Kollaborationsanforderung abgeleitet. SC-004 (95 % Änderungserhalt) bleibt ohne repräsentativ definiertes Konfliktszenario und Messung offen. Bei gleichzeitig unvereinbaren Änderungen am selben Feld kann LWW nicht beide Werte als aktuellen Feldwert erhalten.

## D3: Messgrößen und Timing

Für jede Messung den Originalwert und die Original-ID aus dem [Register](SPEC_REQUIREMENTS_MATRIX.md) übernehmen. Messgrenzen werden nicht zu einem einzigen FPS- oder Latenzwert zusammengezogen:

| Gegenstand | Beginn / Ende der Messung | Ziele und Nachweis |
|---|---|---|
| Offline-Analyse | akzeptierter Analyseauftrag bis persistiertes, lesbares Ergebnis; Kalt- und Warmcache getrennt | Specs 3/6/21 gesondert nach Laufzeit und Genauigkeit; vierminütiges Corpus und Randfälle, parallel zum Abspielen auf Störungen prüfen. |
| Beat-/Tonarterkennung | unabhängig annotierte Quelldatei gegen Ausgabe | BPM-Abstand, Oktavfehler, Beatfehler und Key-Korrektheit getrennt auswerten; Unsicherheit und nicht erkannte Tracks mitberichten. |
| Trigger/Sync | physisches Eingangssignal bis messbarer Audioausgang; zusätzlich interne Zeitstempel | Interface, Treiber, Rate und Buffer angeben; Median, 95./99. Perzentil, Maximum und Messaufbau dokumentieren. Originalgrenzen der Specs 4/16–19 bleiben maßgeblich. |
| Oberfläche | Ereignis bis sichtbarer Frame bzw. Zeitabstand sichtbarer Frames | Playhead, Tooltips, Pad-Feedback und Video einzeln; Specs 7/8/16 nicht durch eine gemeinsame Durchschnitts-FPS ersetzen. |
| Streaming | Aufnahmezeitpunkt, Encoder, gesendeter Stream und plattformseitige Beobachtung | Bitrate, Framedrops, Wiederverbindung und Overlay-FPS getrennt von der Video-Deck-Ausgabe messen; Anforderungen aus Spec 15 anwenden. |
| Mastering / Ressourcen | Eingangs- und Ausgangssignal; Callback-Anfang bis Ende | Signalpegel, True Peak, LUFS, Artefakte, CPU, Deadline-Verfehlungen und zusätzliche Latenz je Konfiguration; Look-ahead separat ausweisen. |
| Adoption / Quoten | definierte Nutzerkohorte und Beobachtungszeitraum | Stichprobengröße, Ein-/Ausschlüsse, Nenner und Rohdaten vorab festlegen; automatisierte Tests ersetzen keine Nutzerbefragung oder mehrmonatige Beobachtung. |

Bei widersprüchlich wirkenden Grenzen zunächst gleiche Messbedingungen definieren; sind zwei Werte für dasselbe Szenario anwendbar, beide unverändert prüfen. Beispiele: Spec 3 fordert für den typischen Track unter 10 Sekunden; Spec 21 nennt 10–30 Sekunden. Diese Formulierungen sind vor endgültiger Annahme der Zeitmessung zu klären, statt automatisch eine zu streichen. Zielwerte und experimentelle Daten müssen separat ausgewiesen werden.

## D5: Architekturentscheidung – native Adaption

**Entscheidung:** Bestehender Rust-Workspace und das derzeit eingesetzte Slint-Frontend sind Ausgangspunkt, mit getrennten Grenzen für UI, Anwendungslogik, Domain und Infrastruktur; Konstruktion am Composition Root. Alte technische Vorgaben wie Electron-Updater, Web Audio, Canvas und IndexedDB beschreiben Referenzverhalten und sind vor Umsetzung durch eine funktional und betrieblich gleichwertige native Lösung zu ersetzen. Keine neue Framework- oder Providerabhängigkeit ist damit pauschal ausgewählt oder freigegeben. Pro Feature dokumentieren: Schnittstelle, Plattformen, Lizenz, Migration, Tests und Grenzen. Realtime-Audio: vorbereitete begrenzte Daten, keine blockierende I/O, Locks oder Heap-Allokationen im Callback; Übergabepfade und Worst-Case-Budget messen. Falls Slint eine spezifizierte Funktion nicht trägt, eigene evaluierte Alternative samt Entscheidung statt stiller Funktionskürzung.

## D6: Testumgebung und offene Voraussetzungen

Aktuell nachgesehene Entwicklungsumgebung: Windows 11 Enterprise 64 Bit, Version `10.0.26300`, Rust `1.99.1`, Cargo `1.99.1`. Der [letzte Workspace-Test](G01_BASELINE_EVIDENCE.md) lief auf HEAD `b783a98`: 504 bestanden, sechs ignoriert; dies ist kein aktueller Hardware- oder cross-platform-Nachweis.

**Vor der jeweiligen Abnahme festzulegen und zu erfassen:** exakte CPU/RAM/GPU, Netz, Stromprofil, Treiber, Geräte, Samplerate, Buffer, OS und Release-Build; reproduzierbares Musik-Corpus mit Lizenzen und unabhängigen Referenzannotationen für BPM, Beats, Tonart und Phrasen; Regression für unbekannte bzw. unanalysierbare Tracks; gekennzeichnete Kalt-/Warmcache-Läufe, Ausreißer und Wiederholungen. Für Windows/macOS und weitere unterstützte Plattformen separate Protokolle; bei MC4000/Audio zusätzlich reale Geräte, vierkanaliges Cue-Routing und Hörprüfung. Die konkreten Geräte, Corpus-Zusammensetzung und Kohorten sind noch nicht festgelegt: D6 ist als vollständiges Abnahme-Gate weiterhin offen.

## Ergebnis / Grenzen

D1–D3 und D5 besitzen jetzt einen konkreten Arbeitsvertrag und reproduzierbare Testideen; die ausdrücklich genannten inhaltlichen Konflikte und ihre endgültige Akzeptanz sind weiterhin offen. D6 benötigt reale Umgebung und Daten. D4 ist G02 und wurde hier nicht als erledigt ausgewiesen. Die Einzelprüfung aller 418 FR und 200 SC läuft weiter; G01 nicht als vollständig markieren.
