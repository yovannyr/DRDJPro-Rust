# Hilfe und Diagnose

Wenn ein Deck stumm bleibt, eine Datei fehlt oder ein Fehlerbericht erscheint, helfen die folgenden Schritte. Die Bedienelemente liegen in der veröffentlichten Testversion **0.1.0-rc.1** teilweise an anderen Stellen als im Entwicklungsstand auf `main`.

| Gesucht | RC1 | Entwicklungsstand auf `main` |
| --- | --- | --- |
| Audioausgang | **AUDIO** | Zahnrad → **Audio** |
| MIDI-Verbindung | **RECONNECT MIDI** | Zahnrad → **Controller** → **RECONNECT MIDI** |
| Absturzberichte | **REPORTS** in der unteren Leiste | **Mehr → REPORTS** |
| Integrierte Hilfe unter Windows | **HELP** | **Mehr → HELP** |

### Das Deck spielt, aber ich höre nichts

1. Beobachte, ob die Zeitanzeige des Decks läuft. Hebe den Kanalfader des spielenden Decks an und stelle den Crossfader für den Test in die Mitte. Beginne mit niedriger Lautstärke am Verstärker oder Lautsprecher.
2. Öffne die Audioeinstellungen. Unter **CURRENT** siehst du das aktive Gerät. Falls dein Interface fehlt, schließe es an und wähle **REFRESH**.
3. Pausiere beide Decks. Wähle das Gerät und **APPLY NOW**. **SAVE FOR NEXT START** merkt es für den nächsten Start vor. Prüfe bei mehreren Ausgängen, ob das gewählte Master-Paar mit deinen Lautsprechern verbunden ist.

### Die Bibliothek zeigt „MISSING“

Schließe zuerst das Laufwerk mit der Musik wieder an. Ist die Datei an einen anderen Ort umgezogen, wähle ihren fehlenden Eintrag in der Bibliothek, klicke **Relocate** und wähle dieselbe Musikdatei am neuen Ort. Falls ein Deck den alten Pfad noch geladen hat, wirf den Track dort zuerst mit **EJECT** aus. So bleiben Crate-Zuordnung, Cues und gespeicherte Loops mit dem Eintrag verbunden.

### Die Bibliothek meldet einen Speicherfehler

Prüfe den freien Speicherplatz und ob dein Benutzerkonto im Bibliotheksordner schreiben kann. Erscheint **Retry save**, klicke darauf und prüfe die Statusmeldung erneut. Bei **session only** können Änderungen beim Beenden verloren gehen. Sichere die vorhandenen Bibliotheksdateien, bevor du eine ältere Sicherung zurückspielst. Die Ordner für jedes System stehen unter [Installation und Updates](Installation-und-Updates.md).

### Mein Controller reagiert nicht

Für den MC4000 prüfst du zuerst Stromversorgung und USB-Verbindung. Wähle dann **RECONNECT MIDI** und lies den Verbindungsstatus. Wenn der Controller als USB-Gerät erscheint, die Decks aber stumm bleiben, kontrolliere zusätzlich das aktive Audiogerät und das Master-Ausgangspaar.

### Nach einem Absturz

Öffne **REPORTS** und wähle den Eintrag mit der passenden Uhrzeit. Die Detailansicht zeigt unter anderem App-Version, Betriebssystem, Prozessorarchitektur und einen bereinigten Aufrufstapel. Bei ausstehenden Berichten kann die App das Fenster beim nächsten Start selbst öffnen. Automatisches Teilen ist anfangs ausgeschaltet. Binäre Speicherabbilder bleiben auf deinem Rechner.

| Aktion | Wirkung |
| --- | --- |
| **CLOSE** | Schließt das Fenster. Eine ausstehende Entscheidung kannst du später treffen. |
| **KEEP LOCAL** | Der ausgewählte Bericht bleibt lokal. Weitere Sendeversuche für ihn werden unterbunden. Eine schon laufende Übertragung lässt sich nicht zurückrufen. |
| **SEND TEXT** | Erlaubt die Übertragung des bereinigten Textberichts an das gespeicherte HTTPS-Ziel. Prüfe die angezeigte Adresse. Der Empfang ist bestätigt, wenn **Server confirmed** erscheint. |
| **PROCESS QUEUE** | Versucht erneut, bereits freigegebene Berichte zuzustellen, etwa nach einer unterbrochenen Verbindung. |
| **DELETE LOCAL** | Löscht den ausgewählten Bericht samt zugehörigem Speicherabbild auf diesem Rechner. |

Zum Senden brauchst du die Adresse eines kompatiblen HTTPS-Empfängers. Trage sie unter **HTTPS destination** ein und wähle **SAVE SETTINGS**. Standardmäßig ist das Feld leer. Mit **Automatically send pending and future text crash reports** gibst du auch ausstehende und künftige Textberichte frei. Wenn du die Option später deaktivierst und speicherst, behalten bereits eingereihte Berichte ihre Freigabe. Für einen solchen Bericht kannst du **KEEP LOCAL** wählen, solange der Empfang noch aussteht.

### Einen Fehler melden

Suche zuerst, ob jemand denselben Fehler bereits gemeldet hat. Für einen neuen Eintrag genügen meist die genaue Paketversion oder der verwendete Entwicklungsstand, das Betriebssystem, das Audio- oder Controller-Modell und die kürzeste Folge von Schritten, nach denen das Problem auftritt. Ergänze, was du erwartet hast, was tatsächlich passiert ist und welche Meldung die App zeigt. Das Issue ist öffentlich. Entferne persönliche Dateipfade aus Text und Screenshots. [Fehlermeldungen auf GitHub](https://github.com/yovannyr/DrDJPro/issues)

[Installation und Updates](Installation-und-Updates.md) · [Handbuchübersicht](Home.md)
