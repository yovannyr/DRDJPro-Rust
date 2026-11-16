# Spec 26 — gemeinsame visuelle und manuelle Abnahme

Entwicklungsstatus: **Erledigt auf Nutzerentscheidung**.
Abnahmestatus: **Gestartet; noch keine gemeinsam bestätigten Ergebnisse**.

## Testbasis

- Softwarestand: `4e28f91`, lokales vorhandenes Windows-Debug-Binary.
- Release-Softwaretests separat: CI #254, Attempt 3, drei Plattformen grün.
- Vorhandene Benutzerberichte und Sharing-Einstellungen werden nicht verändert.
- Für spätere zustandsändernde Fälle werden ein isolierter Teststore und eine
  eigene Testkonfiguration verwendet; insbesondere M06–M09 ändern keine
  vorhandenen Sharing-Einstellungen oder Benutzerberichte.
- Kein Versand an einen produktiven Server im Rahmen der ersten Sichtprüfung.

## Gemeinsame Prüfschritte

| ID | Aktion | Erwartung | Ergebnis |
|---|---|---|---|
| M01 | App starten und Hauptfenster ansehen | App reagiert, Layout lesbar, kein unerwarteter Crash-Dialog | Vom Assistenten beobachtet; gemeinsame Bestätigung offen |
| M02 | `REPORTS` öffnen | Separates Report-Fenster sichtbar; Listen-/Detail-/Statusbereiche und Schaltflächen lesbar | Fenster geöffnet; Kontrastbeobachtung unten, Nutzerbestätigung offen |
| M03 | Fenster verkleinern/vergrößern und Tastaturfokus prüfen | Keine abgeschnittenen zentralen Aktionen; verständlicher Fokus | Offen |
| M04 | Eigenen Testbericht auswählen und Details prüfen | Datum, Version, OS/Architektur und bereinigter Stack passen; Auswahl bleibt stabil | Offen |
| M05 | Post-Crash-Abfrage mit eigenem Testbericht schließen | Entscheidung bleibt ausstehend; kein automatischer Versand | Offen |
| M06 | Nutzer wählt `KEEP LOCAL`, danach App neu starten | Testbericht bleibt lokal und abgelehnt; Entscheidung bleibt erhalten | Offen |
| M07 | Eigenen Testbericht gezielt löschen | Nur ausgewählter Testbericht samt Dump entfernt, sonstige Einträge unverändert | Offen |
| M08 | Nutzer prüft Einstellungen und explizites Speichern | Änderung erst bei Speichern wirksam; ungültiges HTTPS-Ziel liefert verständlichen Fehler | Offen |
| M09 | Autorisierten Textversand gegen vereinbarten Testserver prüfen | UI bleibt bedienbar; Erfolg nur nach ACK; Fehler bleibt wiederholbar | Offen; Testserver nötig |
| M10 | Report-Fenster und Hauptfenster normal schließen | Kein sichtbarer Hänger, kein verwaister Monitor | Offen |

## Erste Sichtprüfung durch den Assistenten

- Vorhandene App ohne Neukompilierung gestartet. Beide Decks bleiben pausiert.
- Hauptfenster bei etwa 1280 × 850: unterer Rand abgeschnitten; nach Maximieren
  auf die sichtbare Desktopfläche ist `REPORTS` erreichbar. M03 bleibt offen.
- Report-Fenster geöffnet: leere Liste, `No report selected`, Logger aktiv,
  kein gespeichertes Upload-Ziel; Aktionen für einen ausgewählten Report deaktiviert.
- Beobachtung: Die Beschriftung der Auto-Sharing-Checkbox wirkt auf dem dunklen
  Hintergrund sehr kontrastarm. Noch keine gemeinsame Bewertung oder Korrektur.
- Keine Sharing-Einstellung gespeichert, kein Report versendet oder gelöscht.

## Unverändert dokumentierte Restpunkte

500-MB-Gesamtspitzenbudget einschließlich Staging, langfristiges Symbolarchiv,
automatische native Text-Anreicherung der UI, produktiver Endpoint/Auth-Vertrag,
erweiterte Release-/Last-/Abdeckungsnachweise und realistische Formulierung
absoluter Verlustfreiheitszusagen sind durch den Statuswechsel nicht umgesetzt.
Die Anforderungsmatrix in `IMPLEMENTATION.md` bleibt maßgeblich für diese Grenzen.
