# Lokale Software Factory

Voraussetzung: Python 3.10 oder neuer. Aufruf aus dem Repository:

```powershell
python tools/software_factory/factory.py generate
python tools/software_factory/factory.py validate
python tools/software_factory/factory.py next
python tools/software_factory/factory.py prepare G01
python tools/software_factory/factory.py start G01
python tools/software_factory/factory.py draft G01
python tools/software_factory/factory.py check G01
python tools/software_factory/factory.py pilot-g01
python -m unittest discover -s tools/software_factory -p "test_*.py"
```

`generate` erstellt das versionierte Register deterministisch aus dem Implementierungsplan; Plan- und Matrix-Hash verhindern die Verwendung nach Dokumentänderungen. Pakete werden erst freigegeben, wenn alle Kind-Slices mit unverändertem Nachweis abgeschlossen sind. `prepare` gibt einen Auftrag aus. `draft` fragt QWEN und danach DeepSeek anhand der Umgebungsvariablen an und gibt nur einen Vorschlag aus; bei Ausfall beider Modelle muss der autorisierte Terra-Fallback im Codex-Host erfolgen. `check` führt die Factory-Tests und bei anderen Slices zusätzlich Formatierung, Workspace-Tests und Clippy aus. Weitere slice-spezifische Tests, Release-Build und Plattform-CI sind separat auszuführen. Der G01-Probelauf erstellt eine erste Bestandsaufnahme und überschreibt keine bestehende Datei.

`block G01 --reason "Grund"` hält eine Blockade in `.software-factory/state.json` fest; `unblock G01` hebt sie nach Klärung auf. `finish G01 --evidence pfad.json` dokumentiert einen Abschluss anhand einer vorhandenen JSON-Nachweisdatei im Repository: `slice`, `open_blockers: []` und `checks` mit mindestens einem Objekt aus `command` und `result: "passed"`. Für G01 müssen außerdem `all_requirements_validated`, `decisions_resolved` und `measurement_plan_defined` jeweils `true` sein. Diese Datei ist eine **menschliche bzw. durch ausführenden Agenten erstellte Bestätigung**, keine automatisch verifizierte Testquittung; vor Eintrag die Nachweise prüfen. Änderungen an der Datei entwerten den Abschluss. Der Probelauf erfüllt diese Bedingungen nicht und schließt G01 nicht ab.

Beispiel für die Struktur einer Abschlussbestätigung (nur nach tatsächlicher Prüfung ausfüllen):

```json
{"slice":"G01", "open_blockers":[], "checks":[{"command":"Prüfbefehl oder Nachweisreferenz", "result":"passed"}], "all_requirements_validated":true, "decisions_resolved":true, "measurement_plan_defined":true}
```

**Grenzen dieser ersten Version:** Modellvorschläge werden nicht automatisch angewendet, Testausgaben nicht als manipulationssichere Quittung gespeichert und Commits/Push nicht automatisch ausgeführt. Der Orchestrator automatisiert damit noch keine vollständige unbeaufsichtigte Implementierung. Keine Zugangsdaten werden in das Register aufgenommen; ihre Werte werden bei Fehlern nicht ausgegeben.
