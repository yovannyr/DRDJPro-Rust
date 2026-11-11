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

- Vollständige Gerätefinder-UI/Composition Root und Einbindung der Geräte-Strings.
- HID-Backend und dokumentierte Controller-Profile.
- Reale DVS-Dekodierung; der vorhandene `dvs.rs`-Prototyp simuliert diese nur.
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
