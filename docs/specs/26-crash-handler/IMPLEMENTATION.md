# Spec 26: Umsetzung und Nachweise

Stand: 2026-11-16. In Arbeit; keine vollständige Abnahme.
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

## Schritt 3 — HTTPS/gzip und dauerhafte Text-Warteschlange

Der Nutzer hat am 16.11.2026 ausschließlich diesen nächsten Slice wieder
freigegeben. Schritt 4 (normaler App-Start, Einstellungen und Slint-Dialoge)
bleibt ein Folgeschritt; visuelle/manuelle Abnahme erfolgt später gemeinsam.
Die früheren Hinweise „HTTP-Transport offen“ beziehen sich auf Schritt 2.

### Vertrag und Datenschutz

`CrashStore::queue_report(id)` erteilt ausdrücklich Zustimmung zum Textversand
einschließlich späterer Wiederholungen. Der persistente Zustand `Queued` ist
selbst die Warteschlange; es entstehen keine zweiten Queue-/Archivdateien.
`Pending` wird nur mit gespeicherter `auto_submit=true` autorisiert; `Declined`
bleibt auch dann lokal, bis erneute explizite Zustimmung erteilt wird.
`Submitted` wird nicht erneut eingereiht. Schon autorisierte `Queued`-Berichte
dürfen unabhängig von der Einstellung für *zukünftige* Reports wiederholt werden.
Für Widerruf vor einem weiteren Versuch den Zustand auf `Declined` setzen.

`spawn_upload_queue(store)` startet einen einzelnen Hintergrunddurchlauf und
liefert ein nicht blockierend abfragbares `UploadTask`. Der Aufrufer bewahrt den
Task auf; Drop fordert Abbruch vor weiteren Requests an und wartet nicht auf
Threads. Das Abbruchsignal wird unmittelbar vor HTTP erneut geprüft. Ein schon
gestarteter Request kann nicht zurückgerufen werden und hat insgesamt 8 s
Deadline (Verbindungsaufbau höchstens 2 s). Keine Arbeit im Audiocallback oder
Panic-/Signalhook; keine Änderung am Capture-/DSP-Pfad.

Der spätere Startup-Aufrufer startet denselben API-Durchlauf erneut. Es gibt
keine laufende Retry-Schleife und keine versteckten HTTP-Retries. Fehler und
Abbrüche lassen `Queued` dauerhaft bestehen. Die separate OS-Dateisperre
`.upload.lock` verhindert gleichzeitige Upload-Durchläufe verschiedener App-
Instanzen. Die Store-Sperre wird niemals während Netzwerk-I/O gehalten.
ACK-Zustandswechsel erfolgen nur, wenn der Bericht noch `Queued` ist;
gleichzeitiges Löschen/Ablehnen wird nicht überschrieben. Ein Wechsel der
Ziel-URL beendet den Durchlauf vor weiteren Requests. Versand ist best effort:
Prozessabbruch nach Serverannahme, aber vor lokaler Bestätigung kann Wiederholung
bedeuten; der Server muss `Idempotency-Key` anhand der Report-ID deduplizieren.

Endpoint: absolute HTTPS-URL ohne URL-Zugangsdaten, Query oder Fragment.
Zertifikats-/Hostnamenprüfung über denselben bereits verwendeten nativen
TLS-Stack wie Apple-Metadaten; Redirects sind vollständig gesperrt. URL und
Serverantwort werden nicht geloggt. Ein realer Endpunkt/Auth-Vertrag ist noch
nicht vorgegeben; externe Nutzerberichte werden in diesem Slice nicht gesendet.

Protokoll v1: `POST`, `Content-Type: application/json`, `Content-Encoding: gzip`,
`Accept: application/json`, `Idempotency-Key: <report-id>`.
JSON enthält `schema_version:1`, `id`, `created_ms`, `version`, `os`, `arch` und
erneut pfadbereinigten `stack`. Generierte Kennungen werden begrenzt und auf
Identifier-Zeichen gefiltert. Keine lokalen Pfade, Queue-Einstellungen oder
Minidump-Dateiinhalte werden angehängt. JSON wird direkt in einen Gzip-Puffer
serialisiert; keine `.gz`-Datei und kein zusätzlicher unkomprimierter Body-Puffer.
Minidumps verbleiben lokal; FR-016 für binäre Anhänge bleibt unqualifiziert.

Nur eine 2xx-Antwort **und** ein höchstens 4096 Byte großes JSON-ACK
`{"id":"<dieselbe-report-id>","accepted":true}` ohne unbekannte Felder bestätigt
den Versand. Auch leeres 204, falsche ID, Ablehnung, ungültiges/zu großes JSON,
TLS-/Netzwerkfehler, HTTP 403/5xx und Redirects bedeuten keinen Erfolg.

### Ressourcen und Nachweise

Keine neuen Dependency-Versionen: `reqwest`, `flate2` und der ausschließlich
für lokale HTTPS-Tests direkt deklarierte `native-tls` waren bereits im Lockfile.
Lokale Builds verwenden das vorhandene `C:/repo/DRDJPro-Rust/target`, bestehende
platzsparende Profile und serialisierte Cargo-Aufrufe. Kein zusätzlicher
Release-Build oder Toolchain-Wechsel für diese Tests, keine automatische Löschung.
Zu Beginn waren auf C: 47,50 GiB frei. Die TLS-Fixtures belegen nur wenige KiB;
die Test-CA wird ausschließlich dem jeweiligen Testclient hinzugefügt.

Automatisierte Testfälle prüfen echten lokalen HTTPS-/gzip-Verkehr, ACK/Status,
Zustimmung/Opt-out, Queue über Neustart, Idempotenz, CA-/Hostnamenablehnung,
Status-/Redirect-/ACK-Fehler, Timeout, Upload-Lock, Abbruch vor HTTP und
Löschen/Ablehnen während eines laufenden Requests. Die unveränderten Store-
und Capture-Tests bleiben Teil der Regression.

Lokale Ergebnisse (Windows, 16.11.2026):

- `cargo +1.99.1 test --locked --lib crash_handler -- --test-threads=1`:
  27 bestanden, einschließlich der 10 neuen Transporttests.
- `cargo +1.99.1 test --locked --lib -- --test-threads=1`:
  1699 bestanden, keine Fehler/Ignorierungen.
- `cargo +1.100.0 clippy --workspace --locked --all-targets --target x86_64-pc-windows-msvc -- -D warnings`:
  Exit 0, keine Warnungsunterdrückung.
- `cargo +1.100.0 fmt --all -- --check` und `git diff --check`: Exit 0.
- Unabhängiges Review/Fixreview: GPT-6.1 Sol, medium, akzeptiert ohne
  verbleibende Befunde. Abbruchprüfung vor HTTP und expliziter Blocking-Modus
  akzeptierter Windows-Testserver-Sockets sind im Follow-up geprüft.
- Der erste HTTPS-Testlauf war wegen geerbtem Windows-Nonblocking-Modus rot;
  die Testserver-Korrektur wurde mit der vollständigen Suite wiederholt.
- C: nach lokaler Prüfung rund 47,00 GiB frei, keine Cache-Löschung. Die
  Differenz zum Anfang ist eine Systemmessung, keine isolierte Cargo-Messung.

Linux-/macOS-Nachweise für den neuen Transport folgen über CI im separaten
gestapelten Slice-PR; keine native/visuelle Abnahme wird daraus abgeleitet.
CI #245 bestand unter Linux vollständig (2410 Tests, 5 vorhandene Ignorierungen),
unter macOS scheiterten zunächst sieben positive HTTPS-Fälle vor dem HTTP-Empfang.
Die ursprüngliche 20-Jahre-Testleaf überschritt Apples dokumentierte 825-Tage-
Grenze. Die Fixture wurde auf 730 Tage (15.11.2026–14.11.2028) mit expliziten
Key-Usages/SKI/AKI erneuert. Laufzeit als tatsächliche Ursache bleibt bis zum
macOS-Folgelauf eine Hypothese. Keine TLS-Prüfung wird deaktiviert; Trust und
Hostname bleiben negative Testfälle. Rotation vor Ablauf ist dokumentiert.
Der neue frühe CI-Transporttest lokalisiert solche Plattformfehler vor dem
Release-Build; die vollständige Workspace-Suite bleibt unverändert Pflicht.
FR-005/006/007/011/SC-008 benötigen weiterhin Startup-/UI-Anbindung;
SC-001–007 benötigen die bereits beschriebenen realen Korpus-/Release-Messungen.
Insbesondere beweist ein lokaler HTTPS-Test keinen Produktionserfolg in 10 s.

## Schritt 4 — Startup und Reportverwaltung

Am 16.11.2026 separat freigegeben: normaler GUI-Startup, Hintergrundanbindung
und Slint-Reportverwaltung mit automatisierten Tests/Review; visuelle/manuelle
Abnahme bleibt ausdrücklich offen. Dieser Abschnitt ersetzt für diesen Slice
die früheren Hinweise „Startup/UI noch nicht implementiert“.

### Lebenszyklus

`CrashStartup::start()` wird nach der Dispatch-Phase interner/Console-Hilfsmodi,
aber vor Audio- und GUI-Erzeugung aufgerufen. Es hält Monitor/Native-Handler bis
nach Audio-/Storage-/GUI-Teardown. GUI und `--demo` sind abgedeckt; interne
Monitor/Probe-Modi starten nicht rekursiv. Console-Decoding/Playback-Hilfsmodi
bleiben ausdrücklich ohne diesen GUI-Lebenszyklus. Monitorinitialisierung nutzt
die vorhandene Fünf-Sekunden-Frist. Storage-/Monitorfehler verhindern normalen
GUI-Start nicht und werden ohne persönliche Pfade sichtbar angezeigt.

Windows ergänzt nur `SEM_NOGPFAULTERRORBOX` zu bestehenden Prozessflags. Ein
Guard stellt das vorherige Suppressionsbit bei Fehler und Shutdown wieder her,
erhält andere spätere Flags und wird vor Freigabe des Monitor-Singletons gelöst.
Die Tests verändern Flags ausschließlich in einem separaten Probe-Prozess.

Nach GUI-Erzeugung liest ein Datei-Worker den begrenzten lokalen Store. Seine
Startup-Antwort löst den bestehenden Upload nur bei gültigem gespeicherten HTTPS-
Ziel und vorhandener Zustimmung aus: `Queued` auch bei deaktiviertem Auto-Modus,
`Pending` nur mit explizitem Auto-Opt-in. Abgelehnte/bestätigte Reports bleiben
ausgeschlossen. Unlesbare Einstellungen sperren Sharing statt Opt-in zu erfinden.
Die Startup-/UI-Brücke verwendet echten lokalen HTTPS-Verkehr in einem Test.

Die UI besitzt den Upload-Task direkt. Main-Window-Close und Session-Drop brechen
weitere Requests sofort per Atomic ab, auch wenn der Datei-Worker auf einer
Store-Sperre wartet. Ein bereits gestarteter Request kann nicht zurückgerufen
werden. Es wird kein Datei-/Netzwerkthread gejoint; der Monitor bleibt während
der nachfolgenden Audio-/Storage-Freigabe aktiv. Keine Änderung am DSP-/Audio-
Callback, keine zusätzlichen Callback-Allokationen/Locks. Native Crash-Capture
benötigt weiterhin den bereits dokumentierten begrenzten Speicher-Snapshot-Wait;
eine harte Audio-Echtzeitgarantie für Crashfälle wird damit nicht behauptet.

### UI und Fehlerverhalten

Separates `REPORTS`-Fenster mit Liste, stabiler ID-Auswahl, Zeitstempel, Version,
OS/Architektur, bereinigten Stackdetails und dauerhaftem Zustandsstatus. Pending-
Reports ohne gültigen Auto-Modus öffnen beim Start eine nichtmodale Abfrage.
Schließen ändert die Zustimmung nicht; keine Playback-/Deckaktionen werden
ausgelöst. `SEND TEXT`, `KEEP LOCAL`, `DELETE LOCAL`, `PROCESS QUEUE`, Refresh und
HTTPS-/Auto-Einstellungen sind über begrenzte Worker-Kommandos angebunden.
Nur `SAVE SETTINGS` persistiert Eingaben; gespeichertes Ziel und bearbeiteter
Entwurf werden getrennt angezeigt. Ungültige URLs oder Auto ohne gültiges Ziel
werden nicht gespeichert. Änderung/Löschung/Ablehnung fordert zuerst Abbruch des
aktuellen Upload-Durchlaufs an. Bereits bestätigte Transfers können nicht durch
Ablehnen/Löschen rückgängig gemacht werden; die UI sagt das ausdrücklich.

Antworten tragen Request-IDs; veraltete Antworten lösen keine Busy-Sperre.
Pro UI nur ein ausstehender Dateibefehl. Listenupdates erhalten die Auswahl über
Report-IDs. Unverwandte Upload-Antworten überschreiben keine Einstellungsentwürfe.
Details/Labels sind begrenzt; es gibt keine Datei-/TLS-/Storearbeit im GUI-Timer.
Kanäle sind begrenzt, Worker werden nicht pro Frame gestartet. Das Hauptfenster
schließt die Reportverwaltung über den bestehenden Auxiliary-Window-Lebenszyklus.
Ein Store-Refreshfehler behauptet weder Erfolg noch Nichtversand: bekannte
Serverbestätigungen bleiben separat sichtbar; weitere Mutationen/Requests werden
bis zu erfolgreichem Refresh gesperrt. Refresh kann nach Reparatur wiederholen.

### Anforderungsstand, ohne native Abnahme vorzutäuschen

| Anforderung | Softwarestand / verbleibende Grenze |
|---|---|
| FR-001 | Echte GUI-Startup-Capture mit separatem Monitor; Prozess-Panic/native Probes. Nicht für jeden möglichen System-/Datenträgerfehler garantiert. |
| FR-002 | Funktionsnamen und bereinigte Quellorte bei verfügbaren Symbolen; Release-Symbolquote/95-%-Nachweis offen. |
| FR-003 | Version/OS/Architektur im dauerhaften Report und Detailansicht. |
| FR-004 | Vorgegebener Benutzerpfad am GUI-Startup; Tests verwenden isolierte temporäre Stores. |
| FR-005 | Explizites, dauerhaftes Opt-in/Keep-local über UI; korruptes Consent fail-closed. |
| FR-006 | Manueller autorisierter Textversand aus Reportmanager; binäre Anhänge bleiben lokal. |
| FR-007 | Dauerhafte Queue wird beim nächsten erfolgreichen GUI-Start angefragt; Server-/Storefehler bleiben lokal. |
| FR-008 | Pollbare Worker/keine UI-Netzwerkwartezeit; Shutdown ohne Join. |
| FR-009 | Monitor vor GUI/Audio gestartet und als Guard bis zum Teardown gehalten. |
| FR-010 | Main-/Worker-Panics und native Fehler im definierten Probe-Korpus; absolute Crash-Abdeckung offen. |
| FR-011 | Nichtmodale Post-Crash-Zustimmungsabfrage für Pending ohne gültigen Auto-Modus. |
| FR-012 | Start/Fehler/Speichern/Ablehnen/Löschen/Upload-Acknowledge/Retry-Ereignisse; keine URL/Payload-Logs. |
| FR-013 | HTTPS-Texttransport implementiert; kein produktiver Endpoint/Auth-Vertrag vorgegeben. |
| FR-014 | Store begrenzt auf 50 Reports und 500.000.000 echte verwaltete Bytes. |
| FR-015 | Text-Upload gzip im Speicher ohne Archivdateien. |
| FR-016 | Textpfade vor Persistenz/Anzeige/Upload bereinigt; binäre Dump-Privacy bleibt unqualifiziert, kein Dump-Upload. |

SC-008 ist durch headless UI-/Actor-/HTTPS-Tests für die Softwareaktionen geprüft,
aber noch nicht nativ/manuell abgenommen. SC-001/002 benötigen definierten
Crash-Korpus/Release-Symbole; SC-003 einen realen Server. SC-004 bleibt als
absolute Aussage unbeweisbar. SC-005/006 sind für die vorhandenen Prozess- und
Retention-Szenarien getestet, keine Garantie bei beliebigen Systemstörungen.
SC-007 wird durch Nicht-Join und Abbruch getestet; native Shutdown-/Lastmessungen
stehen aus. Alle früher genannten Mess-/Privacy-Grenzen bleiben bestehen.

Keine zusätzlichen Dependencies, Toolchains oder lokalen Release-Profile.
Vorhandenes gemeinsames target und platzsparende Profile werden wiederverwendet;
Cargo lokal seriell. Startfreiraum auf C: 46,69 GiB, nach den lokalen Checks
45,95 GiB (Systemmessung, keine isolierte Cargo-Messung). Keine Cache-Löschung.

Lokale Prüfungen, Windows am 16.11.2026:

- `cargo +1.99.1 test --locked --lib crash_handler -- --test-threads=1`:
  39 bestanden, darunter vier Headless-UI- und zwei echte HTTPS-/UI-Startup-
  beziehungsweise Shutdown-Integrationen (innerhalb der Lib-Tests).
- `cargo +1.99.1 test --locked --lib -- --test-threads=1 --quiet`:
  1711 bestanden, keine Fehler/Ignorierungen.
- `cargo +1.99.1 test --locked --test crash_process -- --test-threads=1`:
  13 bestanden, inklusive Startup-Normalende, früher Panic vor UI/Audio,
  unzugänglichem Store und isoliertem Windows-Flags-Readback.
- `cargo +1.100.0 clippy --workspace --locked --all-targets --target x86_64-pc-windows-msvc -- -D warnings`:
  Exit 0; `cargo +1.100.0 fmt --all -- --check`, `git diff --check`: Exit 0.
- Unabhängiges Code-/Test-/Dokumentations-/Workflowreview: GPT-6.1 Sol, medium,
  akzeptiert ohne verbleibende Befunde nach Windows-Flag-/Fehlerstatus-Korrekturen.

Die frühen CI-Prüfungen bestehen aus zwei getrennten Schritten für Crash-Handler-
Libtests und Prozessprüfungen. So verdeckt unter Windows kein zweiter erfolgreicher
Native-Befehl einen fehlgeschlagenen ersten; die gesamte Workspace-Suite bleibt
danach Pflicht. Plattformweite Nachweise folgen im separaten gestapelten Draft-PR
gegen `codex/spec31-itunes-library`, in den der Upload-PR #4 bereits gemergt wurde.
Keine native/visuelle oder produktive Serverabnahme wird aus diesen Tests abgeleitet.
