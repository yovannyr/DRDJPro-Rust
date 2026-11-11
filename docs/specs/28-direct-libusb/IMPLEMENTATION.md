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

- Geräte-Strings und vollständige Gerätefinder-UI/Composition Root.
- Interface-Claims mit sicherem Release, Kontroll-/Bulk-/Interrupt-Transfers.
- HID-Backend und dokumentierte Controller-Profile.
- Reale DVS-Dekodierung; der vorhandene `dvs.rs`-Prototyp simuliert diese nur.
- Vollständige FR-/SC-Abnahme und Gerätemessungen. Insbesondere werden keine
  Serato-Protokolldetails erfunden und keine Herstellerkompatibilität behauptet.

Spec 28 ist damit begonnen, noch nicht vollständig. Native visuelle, manuelle
und Hardwaretests bleiben für den gemeinsamen späteren Termin offen.
