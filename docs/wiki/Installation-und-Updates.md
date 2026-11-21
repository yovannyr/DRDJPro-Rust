# Installation und Updates

Für **Dr. DJ Pro 0.1.0-rc.1** gibt es vier Installationspakete. Öffne auf der Release-Seite den Bereich **Assets** und wähle die Datei für deinen Rechner. Die Paketnamen enthalten `0.1.0`, der Titel der Veröffentlichung enthält zusätzlich `rc.1`. [RC1 herunterladen](https://github.com/yovannyr/DrDJPro/releases/tag/desktop-preview-0.1.0-rc.1)

| Dein System | Datei unter „Assets“ | Installation und Start |
| --- | --- | --- |
| Windows x64 | `Dr-DJ-Pro-0.1.0-Windows-x64.msi` | MSI öffnen und die Installation abschließen. **Dr. DJ Pro** im Startmenü öffnen. Die Installation gilt für dein Benutzerkonto. |
| macOS ab 14, Apple Silicon | `Dr-DJ-Pro-0.1.0-macOS-Apple-Silicon.dmg` | DMG öffnen, **Dr. DJ Pro** auf **Programme** ziehen und von dort starten. |
| macOS ab 14, Intel | `Dr-DJ-Pro-0.1.0-macOS-Intel.dmg` | DMG öffnen, **Dr. DJ Pro** auf **Programme** ziehen und von dort starten. |
| Ubuntu 24.04 amd64 | `Dr-DJ-Pro-0.1.0-Ubuntu-amd64.deb` | Im Downloadordner mit der Paketverwaltung installieren, etwa mit `sudo apt install ./Dr-DJ-Pro-0.1.0-Ubuntu-amd64.deb`. **Dr. DJ Pro** im Anwendungsmenü öffnen. |

Auf dem Mac zeigt ** → Über diesen Mac** den passenden Prozessortyp. Steht dort ein Apple-Chip wie **M1** oder **M2**, wähle Apple Silicon. Für einen Intel-Prozessor wähle das Intel-Paket.

### Download überprüfen

Windows kann bei RC1 einen unbekannten Herausgeber melden. Die MSI hat keine Herausgebersignatur. Die Mac-App trägt eine Ad-hoc-Signatur und ist nicht von Apple notarisiert. Zum Prüfen der heruntergeladenen Datei steht bei denselben Assets die Datei `SHA256SUMS.txt` bereit.

Führe im Downloadordner den passenden Befehl aus. Vergleiche die gesamte ausgegebene Prüfsumme mit der Zeile für genau diesen Dateinamen in `SHA256SUMS.txt`. Bei einer Abweichung lade das Paket erneut von der Release-Seite.

| System | SHA-256 berechnen |
| --- | --- |
| Windows (PowerShell) | `Get-FileHash .\Dr-DJ-Pro-0.1.0-Windows-x64.msi -Algorithm SHA256` |
| macOS | `shasum -a 256 Dr-DJ-Pro-0.1.0-macOS-Apple-Silicon.dmg` (auf Intel den Intel-Dateinamen einsetzen) |
| Ubuntu | `sha256sum Dr-DJ-Pro-0.1.0-Ubuntu-amd64.deb` |

### Vor einem Update die Bibliothek sichern

1. Beende Dr. DJ Pro regulär, damit ausstehende Änderungen gespeichert werden.
2. Kopiere den Bibliotheksordner an einen anderen Speicherort. Er enthält die Datenbank `library.sqlite3` mit Einträgen wie Crates, Hot Cues und gespeicherten Loops. Übernimm auch die vorhandenen Begleitdateien, darunter gegebenenfalls `library.sqlite3.bak`.
3. Sichere deine Musikordner separat. Der Import merkt sich den Dateipfad. Die Musikdatei selbst bleibt dort, wo du sie abgelegt hast.

| System | Standardordner der Bibliothek |
| --- | --- |
| Windows | `%LOCALAPPDATA%\DrDJPro` in die Adressleiste des Explorers eingeben. |
| macOS | Im Finder unter **Gehe zu → Gehe zum Ordner …** den Pfad `~/Library/Application Support/DrDJPro` eingeben. |
| Ubuntu | `~/.local/share/DrDJPro`. Wenn `XDG_DATA_HOME` einen absoluten Pfad vorgibt, liegt der Ordner unter `$XDG_DATA_HOME/DrDJPro`. |

Wer für die Bibliothek mit `DRDJ_LIBRARY_PATH` einen eigenen Speicherort eingestellt hat, sichert die Datenbank samt Begleitdateien dort.

### Eine neuere Version einspielen

Öffne die Veröffentlichungen und prüfe Versionsnummer, Zielsystem und Hinweise. Der Zweig `main` enthält den Entwicklungsstand seit RC1. Änderungen dort erreichen die installierte App mit einem entsprechend neu erstellten Paket. [Alle Veröffentlichungen](https://github.com/yovannyr/DrDJPro/releases)

Beende die App nach der Sicherung. Öffne unter Windows das neue MSI mit demselben Benutzerkonto. Ersetze auf macOS die App in **Programme** durch die neue aus dem DMG. Installiere unter Ubuntu das neue DEB mit der Paketverwaltung. Öffne danach eine bekannte Crate und lade einen vertrauten Track. Prüfe, ob die Datei erreichbar ist und deine gespeicherten Cues angezeigt werden.

[Zum ersten Mix](Erster-Mix.md) · [Hilfe und Diagnose](Hilfe-und-Diagnose.md) · [Handbuchübersicht](Home.md)
