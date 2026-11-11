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

## Schritt 2 — geprüft

Externer Minidump-Prozess mit den Embark-Crates `crash-handler` und `minidumper`.
Die Native-Callback-Seite verwendet nur vorinitialisiertes IPC und Atomics.
Der Rust-Panic-Hook erzeugt begrenzte, pfadbereinigte Symboltexte ohne beliebigen
Panic-Payload. Er erhält Unwind-/Catch-Semantik. Diese Symbolisierung darf im
fehlerhaften Rust-Thread allokieren; sie findet nie im nativen Signalhandler statt.

Eine Capture-Transaktion hält die Dateisperre bis zur Finalisierung. Partielle
Dumps bleiben temporär; bei Fehler/Abbruch werden sie entfernt bzw. beim nächsten
Storezugriff bereinigt. Fertige Dumps über 10.000.000 Bytes werden verworfen;
die native Zwischenpuffer-/Schreibgröße ist damit noch keine harte Peak-Garantie.

Upstream-IPC benötigt eine kleine vendorte Anpassung für Fristen und wiederholte
Captures nach abgefangenen Panics; siehe `vendor/minidumper/DRDJ-PATCH.md` und
[Upstream](https://github.com/EmbarkStudios/crash-handling).
Ein Verbindungsbesitzer verarbeitet sämtliche PONG-/CRASH_ACK-Empfänge. Ein
vorheriges Design mit konkurrierenden Empfängern wurde im Review verworfen.

Auf diesem Windows-Rechner wurden AF_UNIX-Verbindungen in Benutzer-Temp mit
WSAEINVAL reproduziert, während isolierte Kontrollen unter ProgramData funktionierten.
Windows verwendet deshalb einen zufälligen kurzlebigen IPC-Unterordner dort;
eine geschützte Besitzer-DACL wird atomar bei der Erstellung gesetzt. Berichte
bleiben weiterhin am vorgegebenen Benutzerpfad. Es werden keine Systemrichtlinien
oder Geräteeinstellungen verändert. Unix verwendet ein privates 0700-Tempverzeichnis.

Die ersten Prozessläufe scheiterten an der Initialisierung und sind keine
Capture-Nachweise. Nach den Korrekturen wurde die vollständige Prozesssuite
erfolgreich wiederholt:

- `cargo test -p drdjpro-ui-spike --test crash_process --locked -- --nocapture --test-threads=1`:
  9 bestanden, inklusive echter nativer Zugriffsverletzung, Main-/Worker-Panic,
  zwei Worker-Panics plus nativem Fehler im selben Monitor, konkurrierenden
  Fehlern, Neustart nach Catch/Unwind und normalem/abruptem Ende.
- Der Parser prüft echte Minidump-Thread-/Exception-Streams; keine Textdatei wird
  als Dump ausgegeben. Gemessene Probe-Wallzeiten: Main-Panic ca. 1,00 s,
  nativ ca. 0,63 s, Worker ca. 0,96 s im ersten erfolgreichen Lauf.
- `cargo test -p drdjpro-ui-spike --lib --locked`: 1664 bestanden, darunter
  ACL-Readback, verwaiste/live IPC-Verzeichnisse, IPC-Empfangsfrist und Capture-Lock.
- `cargo clippy -p drdjpro-ui-spike --all-targets --no-deps --locked -- -D warnings`:
  Exit 0, bekannte Warnung der unveränderten Spec-27-Abhängigkeit bleibt sichtbar.
- Ein vorhandener App-Clippy-Befund in `cover_art.rs` verwendet nun `div_ceil`
  statt manueller aufrundender Division; alle 13 Cover-Art-Tests bestanden.
- Unabhängiges Review: GPT-6.1 Sol, medium; Protokoll-/Lebensdauerbefunde behoben,
  Follow-ups ohne weitere blockierende Befunde. Logs `C:/Temp/drdj-spec26-s2-*.log`.

Nach einem Abbruch zurückgebliebene Windows-IPC-Verzeichnisse werden beim nächsten
Start nur im reservierten Namensraum bereinigt, mit begrenztem PID-Marker und
konservativer Lebendigkeitsprüfung. Unbekannte Dateien und andere Benutzer bleiben
erhalten. Cleanup ist nie rekursiv.

Linux-/macOS-Runtime, Release-Symbolquote und Prozentziele sind noch nicht gemessen.
Der 4-s-Helper-Watchdog mit 5-s-Clientfristen ist ein begrenzter Fehlerpfad, keine
harte Echtzeitgarantie unter beliebiger Scheduler-/Datenträgerstörung. Die normale
App-Startup-Anbindung folgt mit der UI; bisher verwenden die Prozess-Probes den
Monitor explizit. HTTP-Transport und UI sind weiterhin offen.
