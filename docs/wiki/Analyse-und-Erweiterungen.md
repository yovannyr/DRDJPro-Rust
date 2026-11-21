# Analyse und Erweiterungen

Die Desktop Preview kann lokale Musik ohne Zusatzmodelle auf Tempo, Tonart und Lautstärke untersuchen. Für deinen ersten Titel brauchst du nur die Library und **Analyze Files**. Modellgestützte Beat-Erkennung, Stem-Export und Bibliotheksassistent haben jeweils eigene Voraussetzungen.

## Einen Titel für ein Set vorbereiten

1. Importiere deine Musik über **Files → Add files** oder **Add folder**.
2. Klicke auf **Analyze Files**. Die App wechselt zu **Prepare** und bearbeitet die verfügbaren Titel der gesamten Library, denen eine Analyse fehlt oder deren vorherige Analyse als fehlerhaft markiert ist. Dafür brauchst du keinen Titel auszuwählen. Die Fortschrittszeile nennt verarbeitete Dateien und Fehler. Über **Cancel** kannst du den Durchlauf beenden. Fertig analysierte Titel behalten ihr Ergebnis.
3. Markiere in **Prepare** den gewünschten Titel. Die Wellenform zeigt den Lautstärkeverlauf. Dazu erscheinen die BPM-Schätzung, die Tonart in Camelot-Schreibweise (z. B. **8A** für a-Moll), die empfohlene Gain-Korrektur in dB und der Status der Audiodekodierung. **Unavailable** bedeutet, dass die Analyse für den betreffenden Wert kein Ergebnis ermitteln konnte.
4. Mit **Add phrase cues** setzt du vorgeschlagene Abschnittswechsel als Hot Cues in freie Plätze. Die Vorschläge beruhen auf Klang- und Lautstärkeänderungen. Höre die Positionen beim Vorbereiten an, besonders vor einem geplanten Übergang. Vorhandene Hot Cues bleiben erhalten.

Die Phrasenfunktion verwendet ein gespeichertes konstantes 4/4-Beatgrid oder, falls keines vorliegt, die analysierte Tempobasis. Enthält das gespeicherte Grid Tempo- oder Taktwechsel oder Warp-Marker, meldet die App die Einschränkung. Du kannst die Hot Cues in diesem Fall selbst setzen. Ein von der Analyse vorgeschlagenes Beatgrid lässt sich im Deck prüfen, anpassen und ausdrücklich speichern.

## Welche Erweiterungen sind in meiner Version verfügbar?

| Funktion | Voraussetzung und Bedienung |
| --- | --- |
| **Analyse und Phrasen-Cues** | Im RC1-Installationspaket enthalten. Rufe **Analyze Files** auf und wähle den Titel in **Prepare**. |
| **Neuronales Beatgrid** | Ein separat mit `ml-import` erstelltes Programm, die unterstützten Beat-Modelle, die ONNX-Laufzeit und FFmpeg sind erforderlich. Beim Import oder der Analyse kann ein geeigneter Track den Hinweis **Neural grid BPM proposal** erhalten. Die erste ermittelte Takteins dient dann als Ausgangspunkt für einen neuen Grid-Entwurf. Bei unregelmäßigem Tempo kann die klassische Schätzung erscheinen. |
| **Vier Stems** | Ebenfalls mit `ml-import`, separat installierten Stem-Modellen, ONNX-Laufzeit, FFmpeg und beschreibbarem Cache. Die Vorbereitung trennt Gesang, Schlagzeug, Bass und übrige Instrumente offline. Der Befehl `--stem-mix` exportiert eine Mischung mit vier einstellbaren Anteilen. Die Decks spielen die reguläre Audiodatei ab. |
| **Similar und Transition** | Diese Aktionen erscheinen in **Prepare**, wenn der lokale Python-Bibliotheksassistent eingerichtet ist. Sie funktionieren unabhängig vom Build mit `ml-import`. |

Die veröffentlichten RC1-Installationspakete verwenden die Standardkonfiguration ohne `ml-import`. Für eigene Builds sind die Modellpfade `DRDJ_BEAT_MODELS` und `DRDJ_STEM_MODELS`, die Laufzeit `DRDJ_ORT_LIBRARY` sowie für Stems der Cache `DRDJ_STEM_CACHE` dokumentiert. Die Modelle werden separat installiert. [Einrichtung der Analysemodelle und des Stem-Exports](https://github.com/yovannyr/DrDJPro/blob/main/docs/adr/AI-PREPARATION.md)

## Einen Folgetitel vorschlagen lassen

Der lokale Assistent benötigt Python ab 3.11 sowie `library_assistant.py` und `library_similarity.py` im selben Ordner. Setze vor dem Start der App `DRDJ_LIBRARY_ASSISTANT` auf den absoluten Pfad zu `library_assistant.py`. Die Vorschläge verwenden die gespeicherte Bibliotheksdatenbank und die vorhandenen Analysedaten.

- **Similar** sucht zu einem markierten Titel einen Kandidaten. Ohne zusätzliches Merkmalswörterbuch orientiert sich der Vergleich am Tempo. Mit einem vorbereiteten Wörterbuch (`DRDJ_EMBEDDING_DICTIONARY`) können auch Merkmale wie Tonartmodus und Pegelverlauf in den Vergleich eingehen.
- **Transition** schlägt einen Folgetitel samt Begründung vor. Dazu nutzt der Assistent verfügbare Angaben zu Tempodifferenz, harmonischer Passung und möglichen Aus- und Einstiegspunkten. Die App zeigt den Vorschlag in der Statuszeile. Laden und Starten übernimmst du selbst.

Für Anwendungen mit **Model Context Protocol (MCP)** kann derselbe Assistent separat als lokaler Dienst gestartet werden. Der Client erhält Lesezugriff auf die gespeicherte Library und kann Titel suchen, Trackdetails abrufen, ähnliche Titel finden und Übergänge vorschlagen. Das Werkzeug steuert weder die Decks noch die Wiedergabe. Die Anleitung enthält die Startparameter für den Client. [Bibliotheksassistent und MCP einrichten](https://github.com/yovannyr/DrDJPro/blob/main/docs/adr/AI-PREPARATION.md#local-retrieval-transition-suggestions-and-mcp)

## Webansicht und lokale Hilfe

Unter Windows öffnen **WEB** und **HELP** eigene Fenster. In RC1 stehen beide Schaltflächen direkt in der unteren Leiste, im neueren Entwicklungsstand unter **Mehr**. Dazu benötigt Windows die WebView2-Laufzeit ab Version 102. Unter macOS steht der eingebettete Browser derzeit nicht zur Verfügung.

**HELP** zeigt die mitgelieferte Hilfe auch ohne Internetverbindung. In **WEB** gibst du eine HTTPS-Adresse ein und bestätigst mit **GO** oder der Eingabetaste. **BACK**, **NEXT**, **RELOAD** und **HOME** dienen zur Navigation. Webseitenton wird außerhalb des DJ-Mixers wiedergegeben. Für einen Titel auf Deck A oder B brauchst du eine unterstützte lokale Audiodatei.
