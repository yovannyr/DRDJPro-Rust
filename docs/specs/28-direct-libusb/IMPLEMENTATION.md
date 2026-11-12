# Spec 28 — Direkte USB-Unterstützung

## Schritt 1: Read-only Discovery und Worker

`crates/drdj-usb` verwendet `rusb` 0.9.4 als sicheren Wrapper für libusb.
Die in der Spec genannte Kombination „libusb crate 0.9“ ist kein verwendbarer
Cargo-Abhängigkeitsname in dieser Version. Das vendorte libusb wird lokal
gebaut; kein Treiber wird installiert oder ausgetauscht.
API-Referenz: [rusb 0.9.4](https://docs.rs/rusb/0.9.4/rusb/).

`NativeDiscovery` liest Geräte-, Konfigurations-, Interface-/Alternate- und
Endpoint-Deskriptoren. Die Verbindung wird über Bus und Adresse unterschieden;
gleiche VID/PID-Werte werden nicht zusammengefasst. Optional fehlende Portpfade
ändern die Verbindung nicht. Ein Konfigurationsfehler bleibt am Gerät sichtbar.
Der Schritt öffnet keine Geräte, beansprucht keine Interfaces und löst keine
Kontroll-, Bulk-, Interrupt- oder HID-Transfers aus. Deshalb sind Hersteller-,
Produkt- und Serienstrings in diesem Schritt noch nicht implementiert.

`DiscoveryWorker::native` konstruiert den libusb-Kontext im eigenen Thread.
Ein Snapshot bleibt unveränderlich; UI-Zugriffe verwenden ausschließlich
`try_lock`/`try_send`. Refresh-Anfragen werden in einer Queue mit Kapazität 1
zusammengefasst. Polling erfolgt standardmäßig 200 ms nach dem letzten Scan;
das ist kein harter Nachweis einer Hardware-Latenzgrenze. Die tatsächliche
Scan-Dauer kommt zum Intervall hinzu. Sehr kurze Ereignisse zwischen zwei
Scans können nicht als native Hotplug-Callbacks beansprucht werden.
`added`/`removed` beschreiben nur den Unterschied zum vorherigen Scan und sind
keine dauerhafte Ereignisqueue. Langsame UI-Verbraucher vergleichen ihre
zuletzt beobachtete Geräteliste mit dem aktuellen Snapshot.

Fehler bei Initialisierung und Enumeration werden sichtbar und erneut versucht.
Ein fehlgeschlagener oder intern widersprüchlicher Scan behält die letzte
gültige Geräteliste und erzeugt keine künstlichen Unplug-Ereignisse. Bei einem
späteren gültigen Scan werden echte Änderungen wieder berechnet. Profile sind
explizite VID/PID-Mappings; eine erkannte Profilbezeichnung bedeutet keine
Hardwareabnahme oder erfolgreiche Geräteinitialisierung. Die Registry enthält
noch keine unbestätigten Herstellerprofile.

`Drop` fordert Stop an und wartet nicht auf möglicherweise blockiertes OS-I/O.
Nach Rückkehr des laufenden Scans räumt der Worker seine nativen Ressourcen
selbst auf. Es gibt keine erzwungene Terminierung eines nativen Aufrufs und
keine Behauptung einer harten OS-I/O-Abbruchfrist. In Audio-Callbacks wird keine
Discovery gestartet. Die Library-Reexportierung allein startet noch keinen
Worker und ist keine Integration eines bereits sichtbaren Gerätefinders.

Tests prüfen getrennte Geräte mit gleichen IDs, Metadatenänderungen ohne falsche
Disconnects, echte Entfernen-Ereignisse, Registry-Duplikate, Fehler mit
Wiederherstellung, Thread-Besitz, begrenzte Refresh-Anfragen und nicht blockierendes
Drop bei absichtlich blockiertem Backend. Das Example `inventory` dient einer
automatisierten nativen Read-only-Prüfung; es schreibt nur Anzahlen und Dauer,
keine Seriennummern oder Gerätepfade.

Lokale automatisierte Read-only-Probe (Windows, 2026-11-11): 19 Geräte,
47 Interface-/Alternate-Deskriptoren, erster Snapshot nach 31,91 ms einschließlich
Workerstart. Das ist eine Momentaufnahme dieses Rechners, kein kontrollierter
10-Geräte-Benchmark und keine Messung physischer Plug-/Unplug-Latenz.

Verifikation Schritt 1: 7 Worker-/Modelltests bestanden; striktes Clippy der
neuen Crate bestanden. Unabhängiges Review durch GPT-6.1 Sol mit Denkaufwand
Mittel: keine blockierenden Befunde; Snapshot-Ereignissemantik präzisiert.
Striktes Clippy aller App-Targets (`--no-deps -- -D warnings`),
Crate-Formatprüfung und `git diff --check` ebenfalls bestanden.

## Noch offen

- HID-Auswahl und Controller-Mappings im Gerätefinder; USB-Liste und explizite
  Geräte-Stringabfragen sind seit Schritt 5 in der App angebunden.
- Controller-Mappings und dokumentierte Geräteprofile; Transport und gepufferte
  HID-Eingangserfassung stehen bereit, die App-Verbraucher sind noch offen.
- Native DVS-/USB-/HID-Hardwareabnahme; die reale DVS-Softwarepipeline ist seit
  Schritt 8 angebunden. `dvs.rs` bleibt ein nicht verwendeter Prototyp.
- Vollständige FR-/SC-Abnahme und Gerätemessungen. Insbesondere werden keine
  Serato-Protokolldetails erfunden und keine Herstellerkompatibilität behauptet.

Spec 28 ist damit begonnen, noch nicht vollständig. Native visuelle, manuelle
und Hardwaretests bleiben für den gemeinsamen späteren Termin offen.

## Schritt 2: verwaltete I/O-Sessions und Transfers

`IoWorker` besitzt den privaten nativen libusb-Transport. Initialisierung,
Öffnen, Claim, Alternate-Auswahl, Stringabfragen, Transfers und Release erfolgen
auf dessen Thread. `submit` validiert vor dem Enqueue und verwendet eine Queue
mit Kapazität 16. `IoTicket::poll` liefert genau ein Ergebnis ohne Warten.
Volle Queues melden `QueueFull`; nicht akzeptierte Befehle werden nicht ausgeführt.
Backendfehler bei der Initialisierung können durch einen neuen expliziten Auftrag
erneut versucht werden. Writes werden nach Timeout nie automatisch wiederholt.

Eine Session beansprucht genau das angegebene Interface/Alternate der aktiven
Konfiguration. Vendor-/Product-ID werden beim Auflösen einer Bus-/Adressauswahl
erneut geprüft. Das unterscheidet keine identischen Ersatzgeräte mit denselben
IDs; Auswahl und Disconnect-Handling müssen die aktuelle Discovery berücksichtigen.
Der Transport löst keine Kernel-Treiber, setzt das Gerät nicht zurück und
ändert die aktive Konfiguration nicht. Die explizite Alternate-Auswahl wird
einschließlich Alternate 0 über libusb durchgeführt. Pro Transport sind maximal
16 Sessions geöffnet. Session-IDs sind prozessweit monoton und werden auch nach
Worker-Neustart nicht wiederverwendet; sie dürfen nicht über Prozessneustarts
persistiert werden. Fremde, geschlossene oder ungültige IDs liefern einen Fehler.

Bulk-/Interrupt-Endpunkte müssen Richtung, Nummer, Transferart und dem
beanspruchten Interface/Alternate entsprechen. Puffer sind auf 16 KiB begrenzt.
Transfer-Timeouts liegen zwischen 1 und 250 ms; kleinere Werte werden abgewiesen,
damit eine Rundung auf libusbs unendlichen Null-Timeout ausgeschlossen ist.
Kurze Reads und Writes werden als tatsächliche Länge zurückgegeben. Ein natives
`NoDevice` invalidiert die betroffene Session; andere Sessions bleiben erhalten.
Der Besitzer muss beim Entfernen eines Gerätes auch dessen übrige Sessions schließen.

Rohe Standard-Control-Writes sind gesperrt, weil `SET_CONFIGURATION`,
`SET_INTERFACE`, `SET_ADDRESS` oder rohe Halt-Operationen den verwalteten Zustand
umgehen würden. Standard-IN erlaubt nur Query-Requests. Class-/Vendor-Befehle
benötigen ein dokumentiertes Geräteprotokoll; der generische Transport erfindet
keine Initialisierung oder Controllerkompatibilität.

- `ReadStrings`: temporärer Handle ohne Claim, Unicode-Strings mit expliziter
  Sprache und Feldfehlern; ein fehlender Stringindex führt zu keinem Read.
- `SetString`: ausschließlich USB-Stringdeskriptoren Typ 3, Index/Sprache ungleich
  null, maximal 126 UTF-16-Codeeinheiten/254 Bytes. Keine Device-/Configuration-
  Deskriptoränderungen. Unterstützung durch konkrete Firmware ist nicht behauptet.
- `ClearHalt`: ausschließlich passende Bulk-/Interrupt-Endpunkte, über libusbs
  verwaltetes Clear-Halt inklusive Host-Toggle-Zustand; schließt die Session nicht.
- `Close` und RAII: Claim-Release und Handle-Close auf dem Worker, auch bei
  frühem Fehler, Disconnect oder Worker-Ende.

Grundlagen: [USB-Stringdeskriptoren (Microsoft)](https://learn.microsoft.com/en-us/windows-hardware/drivers/usbcon/usb-string-descriptors)
und [libusb-Geräteoperationen](https://libusb.sourceforge.io/api-1.0/group__libusb__dev.html).

Bei Stop werden noch wartende Aufträge abgebrochen; Tickets erhalten `Stopped`.
Ein bereits laufender Auftrag kann noch seinen tatsächlichen Ausgang liefern.
Drop wartet nicht auf OS-I/O. Open/Claim/Release/ClearHalt besitzen keinen vom
Auftrag vorgegebenen libusb-Transfer-Timeout; ein Stringauftrag kann mehrere
einzeln begrenzte Transfers enthalten. Es wird deshalb keine harte Gesamtdauer
oder erzwungene native Abbruchfrist versprochen. Isochroner PCM-/DVS-Empfang ist
in diesem Transfer-Schritt noch nicht implementiert.

18 automatisierte Tests prüfen Discovery, Validierung, Queue-Grenzen,
FIFO-Reihenfolge, Thread-Besitz, kurze Reads, einmalige Antworten, verworfene
Tickets, Init-Wiederherstellung, keine Write-Replays, Stop/Ressourcenfreigabe,
native Fehlertypen, Disconnect-Invalidierung, parallele Session-Allokation und
genaue USB-UTF16-Deskriptorbytes. Das unabhängige GPT-6.1-Sol-Review (Mittel)
fand zwei Policy-/Session-Fehler; beide sind behoben und im Nachreview akzeptiert.
Reale Claims und Transfers wurden nicht auf angeschlossenen Geräten ausgeführt.
Die finale Test-/Clippy-/Formatprüfung nach diesen Korrekturen ist bestanden:
18 Tests, striktes Crate-Clippy, striktes Clippy aller App-Targets
(`--no-deps -- -D warnings`), `cargo fmt -p drdj-usb --check` und `git diff --check`.

## Schritt 3: expliziter HID-Transport

`HidWorker` verwendet HIDAPI 2.6.3 mit dem Standard-Backend auf einem eigenen
Worker. Er nutzt dieselbe geprüfte begrenzte Auftrags-/Ticket-Implementierung,
aber eine getrennte Befehlsnamensdomäne. Der USB-Backend akzeptiert keine
HID-Befehle und umgekehrt. USB-Bulk-Transfers blockieren dadurch keinen
separaten HID-Worker. Die APIs gehören in UI/Control-Worker, nicht in Audio-Callbacks;
sie sind keine Behauptung einer allokations- oder lockfreien Callback-Schnittstelle.

`Enumerate` liefert OS-Metadaten, Busart, Hersteller, Produkt, Seriennummer und
exakte Pfad-/Collection-Identität, ohne einen expliziten Geräte-Open-, Input-
oder Output-Auftrag auszulösen. Unbekannte Geräte werden nicht automatisch
geöffnet. Tastatur-/Maus-/System-Control-Collections werden beim Open anhand
der OS-Metadaten und zusätzlich anhand des Report-Deskriptors abgewiesen.

Ein expliziter Open prüft Pfad, VID/PID, Interface und Usage erneut und liest
den Report-Deskriptor. `ReportLayout` berechnet die Paketlängen einschließlich
Bit-Padding, mehrerer Report-IDs, Global-Push/Pop und getrennter Input-/Output-/
Feature-Reports. Die Maps sind für Aufrufer unveränderlich. Trunkierte Items,
Überläufe, unausgeglichene Collections/Stacks, ungültige IDs, gemischte ID-Modi,
übermäßige Längen und nicht unterstützte Long-Items ergeben einen Fehler.
Der Parser ersetzt keine herstellerspezifische Zuordnung von Reportfeldern.

Einzelne Input-Reads sind explizite Aufträge mit `read_timeout(..., 0)`;
kontinuierliche Erfassung ergänzt Schritt 4. Native API-Paddingbytes bleiben neben
der logischen Reportlänge erhalten. Output/Feature verwenden den von HIDAPI
geforderten Report-ID-Präfix, einschließlich ID 0 für unnummerierte Reports.
Windows-Padding auf die maximale API-Länge wird berücksichtigt. Alle regulären
API-Frames bleiben auf 4096 Bytes begrenzt; Input-/Feature-Lesepuffer haben ein
zusätzliches Prüfbyte. Überlängen werden vor Annahme des Ergebnisses abgewiesen,
statt ein von HIDAPI gekürztes Präfix als gültigen Report auszugeben.

Native HID-I/O-Fehler und fehlerhafte Eingangsreports invalidieren nur die
betroffene Session. Handles schließen vor ihrem Kontext und auf dem Worker.
USB und HID verwenden gemeinsam prozessweit eindeutige Session-IDs; ein
Worker-Neustart lässt alte IDs nicht auf andere Geräte zeigen. Keine Session
wird über Prozessneustarts persistiert. HID-Feature-/Output-APIs bieten nicht
überall frei konfigurierbare native Abbruchfristen; nur der UI-Auftrag bleibt
nicht blockierend. Physische Latenz-/Kompatibilitätsziele bleiben ungemessen.

Automatisierte Metadatenprobe am 2026-11-12: 28 HID-Collections in 208,49 ms.
Es wurden keine Geräte für Reports geöffnet und keine Input-/Output-Reports
angefordert. Die Ausgabe enthielt keine Seriennummern oder Gerätepfade.

28 Tests prüfen USB/HID-Worker, Queue-/Session-Policies, Descriptor-Parsing,
System-Input-Schutz, Report-ID-/Padding-Fälle und Überlängen. Das unabhängige
GPT-6.1-Sol-Review (Mittel) fand die mögliche unbemerkte Eingangsverkürzung;
Prüfbyte, Completion-Validierung und Lease-Invalidierung sind korrigiert und
im Nachreview akzeptiert. Striktes Crate-Clippy und Clippy aller App-Targets
sind bestanden. Auch die finale Crate-Formatprüfung und `git diff --check`
sind bestanden. Reale HID-Report-/Feature-/Output-Tests bleiben offen.

## Schritt 4: gepufferte HID-Eingangserfassung

`HidCapture` öffnet ausschließlich ein explizit gewähltes Nicht-System-Gerät
auf einem eigenen Reader-Thread. Native Input-Reads warten höchstens 5 ms laut
HIDAPI-Auftrag; Output-/Feature-Aufträge laufen nicht auf diesem Reader.
Ein fehlerhafter Treiber kann native Fristen überschreiten. Drop fordert Stop an,
wartet nicht auf den Treiber und lässt Ressourcen anschließend im Reader schließen.

Die Queue hält maximal 256 Reports. Überlauf verwirft den ältesten Report und
erhöht einen sichtbaren Zähler. Disconnect verwirft verbliebene Reports mit
separatem Zähler; erneutes Öffnen erhöht die Verbindungsepoche. Capture-ID,
Epoche, Session und Sequenz unterscheiden alte Daten auch bei wiederverwendeten
Backend-IDs. Verbraucher müssen bei Verlust oder Verbindungswechsel gehaltene
Controls freigeben und Gestenzustand zurücksetzen. Die App-Anbindung steht aus.

Temporäre Verbindungsfehler werden frühestens nach 250 ms erneut versucht;
Policy-, Descriptor- und ungültige Reportfehler bleiben als `Failed` sichtbar.
`try_drain` wartet nicht auf den Queue-Lock, allokiert aber den Ergebnisvektor:
Die Schnittstelle gehört in Control-/UI-Code und nicht in Audio-Callbacks.

Sieben zusätzliche automatisierte Tests prüfen FIFO für 125 synthetische Reports,
expliziten Überlauf, Disconnect/Reconnect mit wiederverwendeter Backend-ID,
permanente Fehler, frühe Validierung und Stop bei blockiertem Testtreiber.
Der Fehlerstatus wird vor potenziell blockierender Ressourcenfreigabe veröffentlicht;
ein Test mit gesperrtem Destruktor sichert das ab. Der Capture-Transport verzichtet
hierzu auf vorzeitiges Entfernen der fehlgeschlagenen nativen Lease im Read-Aufruf.
Unabhängiges Review durch GPT-6.1 Sol (Mittel): Befund behoben, Nachreview akzeptiert.
35 Tests und striktes Crate-Clippy bestanden.
Der 125-Report-Burst ist keine Messung physischer 125-Hz-Hardware oder Latenz.
Es wurden keine realen HID-Reports von angeschlossenen Geräten erfasst.

## Schritt 5: Gerätefinder in der App

Der Button `USB` öffnet eine Liste mit VID/PID, Bus/Adresse und explizitem
Profilstatus. Initialisierung und Scans beginnen erst beim ersten Öffnen;
Discovery läuft danach mit dem vorhandenen Worker weiter. Unbekannte Geräte
bleiben `unmapped`; eine VID/PID-Zuordnung gilt nicht als Hardwareabnahme.
`READ DEVICE STRINGS` fragt Hersteller, Produkt und Seriennummer ausdrücklich
über einen temporären Handle ohne Interface-Claim ab. Zugriffsfehler bleiben
sichtbar. Es gibt keine automatische Initialisierung, Treiberänderung oder Writes.

Die Auswahl folgt der vollständigen zuletzt beobachteten Geräteidentität statt
einer verschiebbaren Zeilennummer. Entfernen/Identitätswechsel verwirft Auswahl
und ausstehendes Ticket. Bei neuer Auswahl kann eine verspätete Stringantwort
keine falsche Beschriftung setzen. Fehlgeschlagene Scans behalten die letzte
Liste, markieren sie als veraltet und sperren weitere Stringabfragen bis zu
einem erfolgreichen Scan. Ein identischer Ersatz zwischen zwei Scans bleibt
ohne zusätzliche Geräteidentität prinzipiell nicht unterscheidbar.

UI-Timer verwenden ausschließlich die nicht wartenden Snapshot-/Ticket-APIs.
Andere Einstellungen schließen den USB-Finder auch in Gegenrichtung; der Finder
schützt vor Klickdurchgriff und sperrt Explorer-Deck-Drops solange er offen ist.
Vier zusätzliche automatisierte Headless-Tests prüfen Hotplug-Auswahl,
Unicode-/Serienstrings, verspätete Antworten/veraltete Listen sowie echte
Panelaktionen ohne Deckänderungen. Kein nativer visueller oder manueller Test.

Die xwax-Lizenz-/Architekturprüfung und die Freigabe für einen GPLv3-kompatiblen
DVS-Build stehen in [DVS_DECISION.md](DVS_DECISION.md). Der Decoder ist in diesem
UI-Schritt noch nicht integriert.

Verifikation: vier Gerätefinder-Tests bestanden; striktes Clippy aller
App-Targets, gezielte Rust-Formatprüfung und `git diff --check` bestanden.
Unabhängiges GPT-6.1-Sol-Review (Mittel) mit akzeptiertem Nachreview nach Behebung
der zunächst einseitigen Panel-Schließlogik. Die 35 USB/HID-Backendtests des
vorherigen Schritts sind separat grün; keine Aussage über alle App-Regressionen.

## Schritt 6: optionaler GPLv3-xwax-Decoder

`drdj-dvs` enthält den auf `be863572137929f70aa3ff0cef3b6b76f4973005`
gepinnten xwax-Decoder mit Herkunft, vollständiger GPL und dokumentierten
Portabilitäts-/Überlaufkorrekturen. Die Root-Funktion `dvs-xwax` schaltet die
Abhängigkeit explizit zu; dies ist noch keine Eingangs-/Playback-Anbindung.
Der kombinierte DVS-Build darf nicht als MIT-only verteilt werden; Details und
Build-Anleitung stehen in `THIRD_PARTY_NOTICES.md` und `crates/drdj-dvs/VENDOR.md`.
Die MIT-Hinweise des eigenen Codes bleiben erhalten. Eine vollständige
Release-Lizenzprüfung ist durch diese Hinweise nicht ersetzt.

Rust besitzt einen opaken C-Decoder exklusiv auf seinem Worker; keine kopierten
C-Strukturlayouts. Initialisierung serialisiert den Aufbau gemeinsam genutzter
Lookup-Tabellen. Die drei angebotenen Editionen (Serato 2nd Ed. A/B, Serato CD)
belegen zusammen rund 21 MiB für die Prozesslebensdauer; Tabellen werden nicht
unter lebenden Decodern freigegeben. Keine allgemeine Ink-/NoiseMap-Kompatibilität
aus einer Sammelbezeichnung abgeleitet. Keine physische Geräteabnahme behauptet.

Der begrenzte Stereo-i16-Decodepfad allokiert nicht und blockiert nicht auf
einem Mutex. Die Konstruktion kann allokieren/loggen und gehört ausdrücklich
nicht in den Audio-Callback. Ungültige/alte Positionswörter, Stillstand und
unsichere Plattenbereiche liefern kein Steuersignal. Bei über 20 ms altem Stamp
wird Signalhistorie zurückgesetzt; Aufrufer müssen Verlust in einen sicheren
Playback-Zustand übersetzen. Native Positionen verwenden den xwax-Wortursprung;
20-Bit-Fenster identifizieren ihr erstes Bit, nicht den letzten Samplezeitpunkt.
Kalibrierung und physische ±10-ms-Messung bleiben eigenständige Abnahmen.

Acht Tests prüfen synthetisch codierte AM-Stereo-Daten: drei Editionen bei
44,1/48/96 kHz, Geschwindigkeit 0,95, Reverse, absolute Sprünge, Silence/Noise/
reinen Ton, Reset, Phono-Pegel, Kanaltausch, Vollbereich/DC, Safe-Area und
Wiedererfassung. Striktes Crate-Clippy und Root-Clippy aller Targets mit
`--features dvs-xwax --no-deps -- -D warnings` sind grün.
Release-Probe (2026-11-12): 7.500 Blöcke à 256 Stereo-Frames/48 kHz,
p50 2,4 µs, p99 2,9 µs. Nur Decoderzeit, keine Eingangs-/UI-/Ausgabelatenz.
Unabhängiges Review durch GPT-6.1 Sol (Mittel): fehlender ursprünglicher
Copyright-Vermerk im abgeleiteten Testgenerator behoben, Nachreview akzeptiert;
Reviewer hat alle acht Tests zusätzlich selbst erfolgreich ausgeführt.

## Schritt 7: Stereo-Audioeingang und Decoder-Worker

Die DVS-Crate kann einen ausdrücklich ausgewählten OS-Audioeingang über CPAL
öffnen. Enumeration läuft auf einem aufrufenden Control-Worker und startet
keinen Stream; fehlerhafte Gerätemetadaten werden neben gesunden Einträgen
gemeldet. Unterscheidung erfolgt über den aktuellen OS-Namen und die geprüfte
Default-Konfiguration; doppelte Namen werden beim Öffnen abgewiesen.
Keine automatische Geräteauswahl oder Wiedereinschaltung. Die App-Auswahl und
Playback-Anbindung folgen im nächsten Schritt; diese API allein steuert kein Deck.

Unterstützt sind f32/i16/u16, 8–192 kHz und 2–32 Eingangskanäle; verwendet wird
ausschließlich Paar 1/2 ohne Summierung. Einmal abgefragte native Konfiguration
bestimmt sowohl die Prüfung als auch den Streamaufbau. Ungültige Sample-Blöcke
und nicht endliche f32-Werte melden Streamfehler. Alle nativen Calls und der
Decoder gehören dem Worker. Zweiphasige Stream-Guards ermöglichen Fehlerpublikation
vor potenziell blockierendem Cleanup, auch nach fehlgeschlagenem `play()`.

Der einzelne Daten-Callback kopiert/konvertiert in einen vorab allokierten
8192-Frame-SPSC-Ring aus Atomics: kein Lock, Kanalversand, Heap-Allocation,
Logging oder Decoderaufruf in diesem Pfad. Volle Puffer zählen verworfene Frames.
Der Worker verwirft überlaufene oder mehr als 20 ms anstehende Daten, setzt
die Decoderhistorie zurück und erhöht eine Verbindungsepoche. Verlust nach
gültigem Timecode erhöht ebenfalls die Epoche, damit langsame Verbraucher
auch einen schon wiedererfassten Ausfall erkennen. Nach 50 ms ohne PCM wird
Signalverlust veröffentlicht. Snapshots sind nicht wartend lesbar, enthalten
Workerzeit statt Hardwareankunftszeit; Verbraucher müssen alte Snapshots
zusätzlich zurückweisen. Drop wartet nicht auf Treiber oder Threadende.

17 automatisierte Tests bestanden: Decoderfälle plus Ring-Grenzen/Stereo/Wrap,
malforme Samples, Vorabvalidierung, Start-/Streamfehler bei blockierendem Cleanup,
Factorypanic, Überlauf und synthetischer Producer→Decoder→Snapshot einschließlich
Verlust/Wiedererfassung und ausbleibender PCM-Daten. Striktes Crate-Clippy grün.
Es wurde kein echter Audioeingang geöffnet. Hardware-Latenzen und Codec-/Treiber-
Puffer sind ungemessen; keine physische Positionsgenauigkeit behauptet.
Unabhängiges GPT-6.1-Sol-Review (Mittel) und Nachreview akzeptiert: doppelte
Konfigurationsabfrage und vorzeitiger Cleanup bei Startfehler sind korrigiert.
Reviewer hat alle 17 Tests mit Doctests ebenfalls grün ausgeführt.
Root-Clippy aller Targets mit `dvs-xwax`, Format- und Diffprüfung bestanden.

## Schritt 8: explizite DVS-Decksteuerung

Der GPL-kompatible Build zeigt im USB-Finder eine DVS-Seite: Eingangs- und
Record-Auswahl, Phono-Pegelerkennung, Kalibrierungs-Offset in ms sowie ARM/STOP
für A/B. Enumeration und Streamöffnung bleiben auf Workern. Derselbe Eingang
kann nicht gleichzeitig beiden Decks zugewiesen werden; zwei getrennte Eingänge
können zwei Decks steuern. Es gibt keine automatische Initialisierung beim Start.
Die Lizenz-/Quellcode-Schaltfläche öffnet einen eigenen Dialog mit dem vollständigen
eingebetteten GPL-Text und den Herkunfts-/Buildhinweisen.

ARM pausiert das Ziel zunächst; nur gültiger Timecode innerhalb von drei Sekunden
aktiviert die Steuerung. Signalverlust, geänderte Eingangs-Epoche, ältere
Snapshots, manuelle Transportkommandos oder Renderer-/Engine-Wechsel entziehen
die Lease. Wiederkehrender Timecode startet kein Deck automatisch. Alte Freigaben
können weder einen neuen Track noch einen übernommenen Maus-/MIDI-Hold stoppen.
Bei Übernahme durch einen manuellen Platter wird die DVS-Generation entzogen.

Der vorhandene feste PCM-Renderer erhält eine eigene externe Betriebsart:
kontinuierlicher signierter Schritt aus gemessener Geschwindigkeit, kleine
Positionskorrekturen über 5 ms, größere Sprünge/Erstposition als direkter Seek
mit gehaltenem Sample-Dezipper. Keine Source-/PCM-Ersetzung je Update; wie beim
Scratch wird Keylock umgangen. Die bestehenden Maus-/MIDI-Scratch-Ziele behalten
ihre 33-ms-Interpolation. Das Audio-Rendering wartet nicht auf den 5-ms-UI-Timer.

Die Ablaufprüfung verwendet 50 ms monotone Zeit, nicht gerenderte Framezahlen.
Zeit wird bei neuem Befehl und spätestens alle 64 Frames geprüft. Eine Generation
bleibt nach Ablauf verriegelt bis zum expliziten ARM; verspätete Updates heben
die Verriegelung nicht auf. Große native Ausgabebursts können deshalb keine
frischen Befehle allein durch Vorausberechnung ablaufen lassen. Positionsanker
werden zur Zeit des aktuellen Outputframes projiziert, damit ein zwischenzeitlich
gelieferter UI-Anker bereits vorausgerenderte Frames nicht zurückspult.
Eine Gerätepufferlatenz-/Clock-Kalibrierung ist damit nicht gemessen oder zugesagt.

PCM-Blöcke tragen zusätzlich einen einmal pro Daten-Callback bestimmten
Zeitstempel. Der Worker verwirft auch kurze, tatsächlich über 20 ms alte Blöcke,
nicht lediglich lange Queues; frische große OS-Callback-Blöcke bleiben nutzbar.
Alter wird nach der Dekodierung nochmals geprüft. Beide Ring-Arrays werden vor
Streamstart allokiert. Zeitabfrage und atomare Stores sind die zusätzliche Arbeit
im Input-Callback; kein Lock, Logging oder neue Heap-Allokation.

18 DVS-Backendtests und zehn neue Root-DVS-Tests sind grün, darunter unregelmäßige
5/10/16-ms-Updates bei 1x, 0,95x und Reverse, 2.048-Frame-Outputbursts,
Wanduhrablauf mit explizitem Rearming, Jump ohne Wiedergabe dazwischenliegender
Audiodaten, A/B-Isolation und manuelle/Track-Übernahme. Der Renderer-Probelauf
(Testprofil opt-level 0, 300×256 Mono-Frames/48 kHz) misst p50 35 µs,
p99 68,2 µs und null Rust-Heap-Allokationen im gemessenen Pfad. Keine native
Ausgabe-/UI-Latenz oder akustische Abnahme aus dieser Messung abgeleitet.

Abschlussregression dieses Schritts: 2.255 Root-/Integrationstests bestanden,
sechs bestehende Qualifikationstests ignoriert, Doctests bestanden. Der vollständige
Lauf wurde mit `--test-threads=1` und einem ausschließlich auf der CLI gesetzten
App-Testprofil `opt-level=0` ausgeführt; produktive/dev Profile blieben erhalten.
Der erste parallele Lauf überschritt im bestehenden Crash-Prozesstest dessen
5-s-Grenze; isoliert bestand er in 0,75 s, im vollständigen sequenziellen Lauf
ebenfalls. Keine Spec-26-Datei geändert oder pausierte Arbeit wiederaufgenommen.
Die breite Regression deckte außerdem bereits vorher abweichende Modern-
Waveform-Zugänglichkeitsnamen auf: separate stabile Decknamen beheben alle acht
betroffenen Tests, ohne sichtbare Beschriftungen zu ändern.
Unabhängiges GPT-6.1-Sol-Review (Mittel) mit mehreren akzeptierten Nachreviews:
UI-Ziel-Stillstand, Jump-Traversal und Framezahl-Timeout sind behoben und durch
die genannten PCM-Tests abgesichert. Auch die Zugänglichkeitskorrektur ist reviewt.
Finales striktes Clippy aller Root-Targets mit und ohne `dvs-xwax`,
Crate-/gezielte Rust-Formatprüfungen und `git diff --check` bestanden.

## Schritt 9: generische HID-Profile und App-Anbindung

Der USB/HID-Finder zeigt getrennte USB-, HID- und (optional) DVS-Seiten.
HID-Enumeration bleibt bis zum ersten expliziten Aufruf inaktiv. LOAD MAPPING
liest ein benutzerdefiniertes JSON-Profil auf einem Worker, ENABLE öffnet genau
die ausgewählte HID-Collection, STOP beendet ihre Steuerung. Ein Profilvergleich
bedeutet nur passende Metadaten, keine Herstellerzertifizierung.

Das Format und ein rein synthetisches Beispiel stehen in
`fixtures/hid-mapping.example.json`: maximal 64 Bindings/64 KiB, genaue
VID/PID/Interface/Usage-Identität, Report-ID und LSB-basierte Felder mit 1–32 Bits.
Offsets schließen die Report-ID aus. Signierte Felder verwenden Zweierkomplement.
Unterstützt sind A/B-Transport-Toggle, absolute Seek-/Pitch-Achsen und relative
Pitch-Encoder mit 0,01–10 Prozentpunkten pro Schritt. Optionale `when`-Bedingungen
im selben Report erlauben beispielsweise die Tip-/Kontaktfreigabe eines Touchpads.
Alle Felder und Bedingungen werden vor dem ersten Event gegen das gesamte
Report-Layout geprüft. Systemtastaturen/Mäuse bleiben ausgeschlossen.

Initial gehaltene Tasten lösen nicht aus: erst Release, dann Press. Reconnect,
Überlauf, Sequenzlücken und veraltete Reports setzen Latches zurück. Reports über
100 ms werden verworfen, maximal 32 pro 5-ms-Abfrage verarbeitet. Absolute Achsen
werden zusammengefasst; relative Schritte gehen einzeln an die zentrale
Controller-Queue und werden nicht aus einem verzögerten UI-Wert berechnet.
Ein Profilwechsel stoppt die alte Erfassung vor dem nativen Dateidialog.
Die GUI aktiviert eine benutzerdefinierte HID-Collection gleichzeitig; bestehendes
MIDI und beide DVS-Eingänge sind unabhängig. Keine bestehenden MIDI-Presets geändert.

45 USB/HID-Tests und sechs gezielte App-/Controller-Tests bestanden, einschließlich
Report-Gates, vollständiger Layoutprüfung, Reconnect-Latches und mehrerer relativer
Encoder-Abfragen ohne UI-Aktualisierung. Unabhängiges GPT-6.1-Sol-Review (Mittel)
und Nachreview akzeptiert; der gefundene relative Pitch-Verlust ist behoben.
Kein physischer HID-Controller geöffnet oder mit Output-Reports beschrieben.
Zusätzliche Abschlussprüfungen: 562 App-Binärtests bestanden (sechs bestehende
Qualifikationen ignoriert), acht Modern-Waveform- und fünf Layouttests bestanden.
Striktes Root-Clippy mit und ohne DVS, USB-Clippy und Format-/Diffprüfung grün.
Der zusätzliche vollständige Release-Build wird separat protokolliert.
