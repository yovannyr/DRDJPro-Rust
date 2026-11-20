# Dr. DJ Pro – Einstellungen und Live-Oberfläche

## Ziel

Ein Zahnrad rechts oben öffnet in Classic und Modern dieselbe Einstellungsansicht. Die bestehenden Controller und Speicherpfade bleiben für ihre Einstellungen zuständig. Das Öffnen oder Schließen darf weder einen Track laden noch die laufende Wiedergabe unterbrechen.

| Ziel | Inhalte und Einstieg |
| --- | --- |
| Decks | Deckansicht, Quantisierungsraster, Einrasten neuer Cues und Auto-Loops sowie Cue-Auslösung am Raster pro Deck. Sync bleibt als Live-Aktion am Deck. |
| Audio | Gerät, Ausgangspaar, Profil und vorhandene Aktionen zum Speichern oder Anwenden |
| Mixer | Vorhörkonfiguration. Der Direktzugriff auf das Vorhören bleibt in der Live-Ansicht. |
| Library & Ansicht | Layout, Spalten einschließlich Cover und Remote-Display |
| Controller | MIDI-Verbindung, USB-Finder, HID und bei aktiviertem Build DVS |
| Library → Files | iTunes-XML importieren |
| Mehr | Web, Hilfe und Fehlerberichte |
| Live | Transport, FX, Sampler, EQ, Vorhören und globales Quantize bleiben erreichbar. Eine verlorene MIDI-Verbindung erhält eine kontextbezogene Wiederverbindung. |

## Ablauf und Abnahme

| Schritt | Stand | Nachweis oder Abnahmekriterium |
| --- | --- | --- |
| 1. Funktionen zuordnen | Erledigt | Die Tabelle oben zeigt den neuen Einstieg für jede verschobene Funktion. |
| 2. Navigation und Bedienelemente verbinden | Erledigt | [Einstellungsseite](../ui/settings.slint), [Hauptfenster](../ui/app.slint) und [Library](../ui/library.slint) verwenden die vorhandenen Aktionen. |
| 3. Bedienwege absichern | Automatisiert geprüft | Die [Layouttests](../tests/ui_layout.rs) decken beide Ansichten ab. Bei 850 Pixeln Fensterbreite wechseln sie durch alle fünf Reiter, ändern das Loop-Rasterverhalten, schließen per Escape und prüfen Deckzustand und Trackauswahl. Hot-Cue-Tasten und Dateiablage sind im Setup gesperrt. |
| 4. Unabhängiges Review | Erledigt | GPT-6.1 Sol mit mittlerem Denkaufwand hat die Änderung geprüft. Auf seinen Hinweis erhielt der aktive Reiter eine Beschreibung für Screenreader. Das Nachreview hat die Beschreibung, die Deck-Bedienelemente und den Regressionstest geprüft und freigegeben. |
| 5. Übernahme nach main | Über Pull Request | Den aktuellen Prüf- und Merge-Stand zeigt der [Pull Request für die Einstellungen](https://github.com/yovannyr/DRDJPro-Rust/pulls?q=is%3Apr+head%3Acodex%2Forganized-settings). |
| 6. Abnahme auf dem DJ-Rechner | Offen | In Classic und Modern die Lesbarkeit auf dem eigenen Bildschirm und die Bedienung per Tastatur prüfen. Während eines laufenden Tracks das Setup öffnen und schließen, über Kopfhörer vorhören und eine getrennte Controller-Verbindung wiederherstellen. |

Bei der lokalen Prüfung unter Windows bestanden 44 ausgewählte Tests mit aktiviertem `ml-import`. Davon entfielen 18 auf Bibliotheksmodule einschließlich USB-Finder, 12 auf Controller und Dateiablage, acht auf moderne Wellenformen und sechs auf die Oberflächennavigation. Formatierung und Clippy mit Warnungen als Fehler bestanden ebenfalls.

Als Vorbild dient die [Gruppierung im Setup von Serato DJ Pro](https://support.serato.com/hc/en-us/articles/234992348-Setup-screen).
