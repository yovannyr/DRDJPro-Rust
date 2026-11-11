# Spec 26: Umsetzung und Nachweise

Stand: 2026-11-11. In Arbeit; keine vollständige Abnahme.
Scope ausschließlich Crash Handler. Specs 27–31 werden nicht umgesetzt.

## Schritte

1. Lokaler Store, gültige IDs/Dateinamen, dauerhafte Zustände, atomare Persistenz,
   sichere Pfadableitung, Privacy-Projektion, 50-Report-/500.000.000-Byte-Grenzen.
2. Separater Monitor, echte Minidumps für Panic/native Fehler, begrenzte
   Initialisierung und Beendigung. Prozess-Tests statt globaler Test-Panic-Hooks.
3. Hintergrund-Upload: explizite Zustimmung, HTTPS, gzip, Fehler/Retry und
   persistente Zustände. Kein Erfolg ohne bestätigte Serverantwort.
4. App-Start und Slint-Reportverwaltung mit Post-Crash-Hinweis, Details,
   Zustimmung, Löschen, manuellem Senden und Konfiguration.
5. Gesamttests, unabhängiges Review und Anforderungs-/Messnachweise.

Jeder Schritt erhält angemessene Tests und ein unabhängiges Review durch
GPT-6.1 Sol, medium. Ursprünglich angefragtes Implementierungsmodell: QWEN. Nach
abgeschnittenen/leeren QWEN-Codeantworten autorisierte der Nutzer die direkte
Implementierung mit GPT-6.1 Sol; diese Auswahl gilt für die folgenden Änderungen.

## Erster Schritt: Akzeptanz und Tests

Betroffene Dateien: `src/crash_handler/`, `Cargo.toml`, `Cargo.lock` sowie
dieses Dokument. Keine Audio-/DSP-Änderung. Persistenz und Dateisperren dürfen
nur außerhalb des Audiocallbacks laufen.

Nachzuweisen: Zustände über Neustarts, eindeutige Windows-kompatible IDs,
beschädigte/unvollständige Metadaten, begrenzte Dateigrößen, Aufbewahrung anhand
realer Dateigrößen, Traversal-/Symlink-Abwehr, Privacy-Projektion, kein
vorgetäuschter Upload-Erfolg. Temporäre Tests arbeiten in eigenen Verzeichnissen.
Zustände unterscheiden ausstehende Zustimmung, abgelehnt/lokal, autorisiert mit
Retry und erfolgreich gesendet; binäre Anhänge brauchen eine eigene Autorisierung.
Gleichzeitige Schreibvorgänge von App und Monitor sowie Wiederherstellung nach
Fehlern sind explizite Tests. Die Speichergrenze verwendet die strengeren
500.000.000 Bytes statt 500 MiB.

## Grenzen der Abnahme

Die absoluten Aussagen „100 % aller Crashes“ und „niemals verloren“ sind bei
Stromverlust, vollem/defektem Datenträger, OOM oder defektem Handler nicht
belegbar. Ergebnisse werden für konkrete Tests angegeben, ohne diese Aussagen
als erfüllt zu markieren. Prozentziele benötigen einen definierten Crash-Korpus.

Binäre Minidumps können private Speicherinhalte und Pfade enthalten. Die
Spezifikationsannahme, sie seien frei von sensiblen Daten, ist keine Garantie.
Textredaktion und Zustimmung zu binären Anhängen werden getrennt behandelt.
Zustimmung allein erfüllt FR-016 für binäre Anhänge nicht: Ohne nachgewiesene
Pfadbereinigung muss diese Anforderung ausdrücklich offen bleiben bzw. dürfen
solche Anhänge nicht hochgeladen werden.
Ein realer Upload-Endpunkt ist noch nicht vorgegeben; Transporttests benötigen
einen lokalen HTTPS-Testserver mit vertrauenswürdiger Test-CA, keinen Versand
echter Nutzerberichte. Plaintext-URLs und unsichere Redirects werden abgewiesen.
Funktions-/Quellortziele brauchen verfügbare Release-Symbole/PDBs und einen
definierten Prozess-Crash-Korpus; Unit-Tests belegen keine 95-%-Quote.

Visuelle/manuelle und Hardware-/Hörabnahme bleiben gemäß Nutzerentscheidung
offen. Headless-Tests werden nicht als native Abnahme ausgegeben.

## Ausgangsstand

Basis `5b764af`. Der ursprüngliche Checkout enthält fremde, uncommittete
USB-Abhängigkeiten einschließlich einer nicht auflösbaren `libusb`-Version.
Die Umsetzung liegt deshalb im isolierten Worktree `spec26-crash-handler`.
Diese USB-Änderungen werden weder verändert noch übernommen.

Vorabreview: undefinierte Queue-Hilfsfunktion, ungültige Windows-Dateinamen,
Upload-Platzhalter mit falschem Erfolg, ungesicherte Metadatenpfade und
synchrone Arbeit/Mutexe im Panic-Hook. Diese gelten als offen bis zur jeweiligen
nachgewiesenen Reparatur.

## Schritt 1 — geprüft

Der ungenutzte Prototyp mit falschem Upload-Erfolg und ungesicherten Metadatenpfaden
ist durch einen lokalen Store ersetzt. Der Store hat noch keine Capture-/Upload-API.
Dump-Dateien zählen bei der Aufbewahrung mit; ein externes Capture benötigt im
nächsten Schritt eine atomare, begrenzte Transaktion statt eines offenen File-Handles.
Reservierte temporäre Schreibdateien werden nach Prozessabbrüchen unter der
Dateisperre entfernt; unbekannte Dateien bleiben erhalten.

- `cargo test -p drdjpro-ui-spike --lib crash_handler --locked`: 12 bestanden.
- `cargo test -p drdjpro-ui-spike --lib --locked`: 1659 bestanden, 0 fehlgeschlagen.
- `cargo clippy -p drdjpro-ui-spike --lib --no-deps --locked -- -D warnings`: Exit 0.
- `rustfmt --edition 2021 --check src/crash_handler/mod.rs`, `git diff --check`: Exit 0.
- Unabhängiges Review: GPT-6.1 Sol, medium; zwei Befunde behoben und Follow-up
  ohne weitere blockierende Befunde.

Die Variante ohne `--no-deps` scheitert an vorhandenen Lints in `drdj-proto`
(Spec 27); diese bleiben gemäß Scope unverändert. Auch die bestehende Warnung
über dessen unbekanntes `drdjpro`-Feature wird nicht unterdrückt. Kein grüner
Workspace-Gesamtstatus. Logs unter `C:/Temp/drdj-spec26-s1-*.log`.
