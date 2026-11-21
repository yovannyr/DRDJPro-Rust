# Library und Vorbereitung

Mit der **Library** findest du lokale Musik und lädst sie auf Deck A oder B. In **Crates** sammelst du Titel für spätere Sets. Die **Prep Queue** ist deine geordnete Auswahlliste für den nächsten Einsatz. Falls die Tabelle im modernen Layout ausgeblendet ist, klicke auf **SHOW LIBRARY**.

## Musik importieren und laden

1. Öffne **Files**. Falls die neuen Titel direkt einer vorhandenen Crate zugeordnet werden sollen, wähle links die gewünschte Ziel-Crate.
2. Wähle **Add files** für einzelne Dateien oder **Add folder** für einen Ordner samt Unterordnern. Die Musikdateien bleiben am ursprünglichen Speicherort. Einen bereits bekannten Dateipfad erkennt die Library wieder.
3. Wähle links **All tracks** und suche nach einem Titel. Markiere die Zeile und klicke auf **Load A** oder **Load B**. Du kannst die Zeile auch auf das gewünschte Deck ziehen. Das Deck lädt den Titel pausiert. Ein laufendes Deck ist gegen Ersetzen geschützt.

Unterstützt werden MP3, WAV, FLAC, AIFF/AIF, M4A mit AAC-LC oder ALAC sowie Ogg/Vorbis und Ogg/Opus. Für einen Ordnerimport gilt eine Grenze von 5.000 unterstützten Audiodateien pro Durchgang. Meldet die App eine Überschreitung, wähle einzelne Unterordner. Den laufenden Import kannst du mit **Cancel** abbrechen.

## Titel finden und Spalten anpassen

**Search library** durchsucht Titel, Interpret, Album, Dateiname und Pfad innerhalb der gewählten Ansicht. Ein Teil des Namens genügt. Die Suche ignoriert Groß- und Kleinschreibung. Mit **X** leerst du das Suchfeld. Findest du einen bekannten Titel in einer Crate nicht, wechsle zu **All tracks**.

| Was du sehen möchtest | Bedienung |
| --- | --- |
| Titel nach Tempo ordnen | Klicke auf **BPM**, ein zweites Mal für die umgekehrte Reihenfolge. Für fehlende BPM-Werte starte **Analyze Files**. |
| Weitere Angaben einblenden | Öffne **Columns** und aktiviere etwa **Cover**, **Key**, **Gain dB**, **Analysis** oder **Path**. Die Pfeile im Dialog verschieben die Spalte. |
| Rechts liegende Spalten lesen | Nutze **◀** und **▶** unter der Tabelle. |
| Weitere Treffer sehen | Nutze **‹** und **›** neben der Seitenzahl. Jede Seite enthält bis zu 200 Treffer. |

## Crates, Verlauf und Prep Queue

| Ansicht | So arbeitest du damit |
| --- | --- |
| **Crates** | Erstelle mit **+** eine Sammlung oder mit **+ Sub** eine Untergruppe der gewählten Crate. Markiere unter **All tracks** einen Titel, wähle im Auswahlfeld links die Ziel-Crate und klicke auf **Add**. Mit **Remove** entfernst du die Zuordnung innerhalb der gewählten Crate. |
| **+ Smart** | Speichere einen Suchbegriff, etwa einen Interpreten oder den Namen eines Musikordners. Passende Titel erscheinen automatisch, auch nach späteren Importen. Im Suchfeld kannst du diese Auswahl weiter eingrenzen. |
| **History** | Hier findest du die zuletzt gestarteten Bibliothekstitel. Jeder Titel erscheint einmal, beim erneuten Start rückt er nach vorn. Eine kurze Cue-Vorschau allein erzeugt keinen Eintrag. |
| **Prepare** | Markiere einen Titel, um Wellenform, Tempo, Tonart und Gain-Empfehlung seiner Analyse zu sehen oder Phrasen-Cues hinzuzufügen. |
| **Prep Queue** | Markiere einen Titel und klicke unter der Tabelle auf **To Prep**. Im Reiter **Prep Queue** ordnest du Titel mit **Up** und **Down** oder entfernst sie mit **Remove**. Laden erhält den Eintrag. Die Reihenfolge wird für den nächsten Start gespeichert. |

Wenn du eine Crate mit **Delete** und **Confirm delete** löschst, bleiben die enthaltenen Titel in der Library. Direkte Untergruppen rücken eine Ebene nach oben.

## Eine verschobene Datei wiederfinden

Steht bei einem Titel **Missing**, prüfe zuerst, ob das Musiklaufwerk angeschlossen ist. Hat sich der Pfad geändert, wähle die betroffene Zeile und klicke auf **Relocate**. Wähle am neuen Ort genau die zugehörige Audiodatei. Falls der Titel noch auf einem Deck liegt, entlade ihn dort vorher mit **EJECT**.

Die Crate-Zuordnungen, der Platz in der Prep Queue, das Beatgrid, die Hot Cues, die gespeicherten Loops und der Verlauf bleiben mit dem Titel verbunden. Verweist die ausgewählte Ersatzdatei bereits auf einen anderen Bibliothekseintrag, meldet die App den Konflikt. Nach erfolgreicher Zuordnung kannst du den Titel wieder laden. Bei einem Speicherfehler erscheint **Retry save**.

## Eine iTunes- oder Music-Mediathek übernehmen

In **Desktop Preview 0.1.0 RC1** öffnest du **iTunes** in der unteren Leiste. Im neueren Entwicklungsstand findest du denselben Import unter **Files → iTunes…**.

1. Wähle im Quellenfenster **CHOOSE XML** und den XML-Export deiner Mediathek. Falls die Mediendateien inzwischen unter einem anderen Stammordner liegen, gib diesen mit **FOLDER** an.
2. Klicke auf **IMPORT / SYNC**. Der Status zeigt unter anderem neue, aktualisierte, fehlende und übersprungene Einträge. Über die Quellenauswahl kannst du eine eingelesene Mediathek später erneut öffnen.
3. Wähle eine Quell-Playlist. Die ursprüngliche Reihenfolge und mehrfach enthaltene Titel bleiben dort sichtbar. Beim Anklicken einer Zeile siehst du die importierten Angaben, etwa Bewertung und Wiedergabezähler. Lokal verfügbare Titel kannst du mit **LOAD A** oder **LOAD B** laden und auch in der normalen Library suchen.

Der Import verarbeitet exportierte XML-Dateien mit Verweisen auf lokale Musik. Fehlende Dateien bleiben als **Missing** sichtbar. Geschützte oder reine Abonnementtitel werden übersprungen. Für den lokalen Import brauchst du keinen Apple-Zugang. **APPLE METADATA** und **APPLE PLAYLIST** fragen auf Wunsch zusätzliche Katalogdaten für einen Titel beziehungsweise bis zu 50 unterschiedliche Playlist-Titel ab. Dafür muss vor dem Start ein MusicKit-Entwicklertoken eingerichtet sein.

Details zu Dateipfaden und Datensicherung findest du in der [Dokumentation zum Bibliotheksspeicher](https://github.com/yovannyr/DrDJPro/blob/main/docs/LIBRARY_STORAGE.md).
