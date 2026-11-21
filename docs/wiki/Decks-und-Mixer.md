# Decks und Mixer

Mit Deck A und B kannst du zwei Titel unabhängig abspielen, vorhören und ineinandermischen. Die Vier-Deck-Ansicht zeigt zusätzlich C und D als schreibgeschützte Plätze ohne Audio. Für den ersten Übergang genügen zwei Titel, die Kanalfader und der Crossfader. Zum getrennten Vorhören brauchst du außerdem einen freien Stereoausgang für den Kopfhörer. [Ausgänge und Kopfhörer einrichten](Controller-und-Anschluesse.md)

## Einen Übergang spielen

1. Markiere in der Bibliothek den ersten Titel und drücke `Umschalt` + `←` für Deck A. Lade den zweiten mit `Umschalt` + `→` auf Deck B. Alternativ öffnet **LOAD** am jeweiligen Deck die Dateiauswahl. Ein spielendes Deck ist gegen Ersetzen geschützt.
2. Schiebe den Crossfader ganz zu A, öffne den Kanalfader A und drücke **PLAY** auf A. Die laufende Positionsanzeige und der Ausgangspegel zeigen dir, ob der Titel bis zum Ausgang gelangt.
3. Schalte unter **MON** die Option **CUE B (pre-fader)** ein und drehe den Kopfhörermix in Richtung **CUE**. Starte B kurz und suche den ersten Schlag deiner gewünschten Passage. Pausiere dort und setze den Einstieg mit **CUE**. Die Vorhörfunktion heißt in den Modern-Kanalzügen **PFL**.
4. Starte B passend zu A. Öffne den Kanalfader B und bewege den Crossfader langsam in Richtung B. Liegen die Schläge knapp auseinander, korrigiere B mit einem kurzen **NUDGE**. Bei zwei verlässlichen Beatgrids kannst du dafür auch **PHASE** oder **SYNC** verwenden.
5. Ist der Crossfader ganz bei B, pausiere A. In Classic entlädt **EJECT** den Titel aus diesem Deck. Sein Bibliothekseintrag bleibt erhalten.

## Einsatzpunkte setzen und wiederfinden

| Bedienelement | So verhält es sich |
| --- | --- |
| **CUE** am Deck | Beim Laden liegt der temporäre Cue-Punkt am Titelanfang. Pausiert an einer anderen Position setzt ein Druck den Punkt neu. Direkt am Punkt startet Gedrückthalten die Vorschau. Beim Loslassen springt das Deck zurück und pausiert. Drücke während der Vorschau **PLAY**, wenn der Titel weiterlaufen soll. Während normaler Wiedergabe führt **CUE** zum Punkt zurück und pausiert. |
| **START** und **STUTTER** in Classic | **START** springt zum Titelanfang. **STUTTER** startet erneut am temporären Cue-Punkt. Der temporäre Punkt gehört zum gerade geladenen Deck und wird beim Titelwechsel zurückgesetzt. |
| Hot-Cue-Pads **1–8** | Ein freies Pad speichert die aktuelle Position für diesen Titel. Ein belegtes Pad springt während der Wiedergabe zur gespeicherten Stelle. Bei pausiertem Deck kannst du die Stelle durch Halten vorhören und mit **PLAY** übernehmen. Beide Decks sehen dieselben Hot Cues, wenn derselbe Bibliothekstitel geladen ist. |
| **SLOT**, **EDIT**, **DELETE** in Classic | Wähle die Nummer eines Hot Cues. **EDIT** öffnet Name und Farbe, **SAVE** übernimmt sie. **DELETE** leert den gewählten Platz. So kannst du etwa den ersten Refrain als „Refrain“ markieren und später am Namen in der Wellenform erkennen. |

**Tastenkürzel.** `1` bis `5` bedienen die ersten fünf Hot Cues von A. `6`, `7`, `8`, `9` und `0` bedienen die ersten fünf von B. Die Plätze sechs bis acht erreichst du jeweils über die Bildschirm-Pads. In einem aktiven Textfeld dienen die Tasten der Eingabe. Hat die Classic-Detailwellenform den Fokus, zoomst du mit `+` für mehr Details hinein und mit `-` für mehr Überblick heraus. Ein Klick in die Wellenform setzt zugleich die Abspielposition.

## Einen Loop vorbereiten

In Classic setzt **IN** den Anfang und **OUT** das Ende. Setze **OUT** später im Titel als **IN**. Drücke **LOOP**, um den markierten Bereich zu wiederholen. Ein Klick auf **ACTIVE** beendet die Wiederholung. **RELOOP** springt zum gespeicherten Anfang dieses Bereichs und aktiviert ihn erneut.

Der kleine Umschalter neben **RELOOP** trägt den Namen der Werkzeugansicht, die du als Nächstes öffnen kannst. Für Loops stehen zwei Werkzeugansichten bereit.

- **AUTO** erzeugt aus einem gespeicherten Beatgrid einen Loop in der gewählten Beatlänge. **½** und **×2** halbieren oder verdoppeln einen vorhandenen Loop. Für vier Schläge im 4/4-Takt wähle **4 beats**.
- **SLOTS** speichert die gesetzten Grenzen pro Titel. Wähle einen der acht freien Plätze, vergib bei Bedarf einen Namen und drücke **SAVE**. Bei der nächsten Verwendung lädt **RECALL** die Grenzen. Mit **LOOP** startest du die Wiederholung. Um einen belegten Platz zu ersetzen, lösche ihn zuerst über **DELETE**.

## Tempo, Beatgrid und Quantisierung

**PITCH** verändert das Grundtempo des Decks. Mit **1×** kehrst du zur Originalgeschwindigkeit zurück. **KEYLOCK** hält beim Tempoändern die Tonhöhe. **HOLD −2%** und **HOLD +2%** in der Classic-Werkzeugansicht **NUDGE** bremsen oder beschleunigen ein spielendes Deck nur so lange, wie du die Taste hältst. Das hilft bei einem kleinen Versatz, wenn beide Titel bereits dasselbe Tempo haben.

| Wenn du das erreichen willst | Bedienung |
| --- | --- |
| B soll das Tempo von A übernehmen und an seiner bisherigen Stelle weiterspielen | **MATCH** unter **PITCH** auf B. Beide Titel brauchen ein gespeichertes Beatgrid mit konstantem Tempo. Reicht der gewählte Pitch-Bereich nicht aus, wähle auf B einen größeren. |
| B soll einmalig auf den Schlag von A springen | Lass beide Decks spielen, beende aktive Loops und drücke **PHASE** unter **NUDGE** auf B. Die Auswahl **Beat** richtet am Schlag aus. Weitere Ausrichtungen stehen im selben Bereich. |
| B soll A auch nach der Ausrichtung weiter folgen | Drücke **SYNC** auf B. A gibt als Referenz das Tempo vor. **SYNC ON** beendet das Folgen. Ist die Funktion gesperrt, prüfe die gespeicherten Grids, den Pitch-Bereich sowie den Wiedergabe- und Loopzustand beider Decks. |

**QUANTIZE** schaltet das Einrasten global ein. Im aktuellen Entwicklungsstand findest du unter **Zahnrad → Decks** die Optionen je Deck. **Cues einrasten** legt neue Hot Cues auf den nächstliegenden gewählten Rasterschritt. **Cue am Raster starten** wartet beim Abruf eines belegten Hot Cues während der Wiedergabe auf den nächsten Schritt. **Loops einrasten** richtet den Anfang eines Auto-Loops am nächstliegenden Beat aus. In der **Desktop Preview RC1** führt der Weg in Classic über **Snap cues** und **Next grid** am Deck sowie **BEAT SNAP** in der Ansicht **AUTO**. Überprüfe bei einem hörbar falschen Einsatz zuerst, ob das gespeicherte Raster auf den Schlägen liegt.

## Pegel und Klang einstellen

Öffne **EQ** in der Mixerleiste. **TRIM** (in Modern **Gain**) regelt den Eingangspegel des Decks. **LOW**, **MID** und **HIGH** verändern Bässe, Mitten und Höhen. Links von der Mittelstellung arbeitet **FILTER** als Tiefpass, rechts als Hochpass. **OFF** neutralisiert den Filter, **RESET** setzt zusätzlich Trim und EQ dieses Decks zurück.

Der **Kanalfader** dosiert den Pegel eines Decks im Mix. Der **CROSSFADER** blendet zwischen A (links) und B (rechts) über. Beobachte bei lauten Passagen beide Anzeigen. **PRE** misst das einzelne Deck nach EQ und Trim und vor den Fadern. **PRE LAST** zeigt den letzten Messwert bei inaktiver Wiedergabe. Steuert bereits **PRE** zu hoch aus, senke **TRIM**. Steigt erst der Ausgangspegel beim Überblenden in den roten Bereich, nimm Kanalpegel oder Effektanteil zurück.

## Effekte einsetzen

Unter **FX** hat jedes Deck drei hintereinander geschaltete Plätze. Wähle im Bereich des gewünschten Decks **Slot 1**, **Slot 2** oder **Slot 3**. Wähle **Delay**, **Low-pass** oder **High-pass** und schalte **Enabled** ein. Die Zusammenfassung über den Reglern zeigt den Zustand der Kette.

Für ein Echo im Rhythmus wähle **Delay → Deck BPM → 1/2 beat**. **Dry/Wet** bestimmt, wie viel Echo hörbar wird. **Feedback** bestimmt, wie stark es sich wiederholt. Fehlt ein geeignetes Grid, kannst du **Milliseconds** für eine feste Zeit oder **Tap BPM** für ein von Hand getipptes Tempo wählen. Tippe **TAP** mindestens dreimal gleichmäßig. Eine Sekunde Halten setzt auf **Deck BPM** zurück. Bei den beiden Filtern steuert **Filter intensity** die Stärke. Unter den Reglern kannst du die gesamte Kette mit **SAVE** auf einem der acht Presetplätze des Decks ablegen und mit **LOAD** abrufen.

## Ein Sample über beide Decks legen

Öffne **SAMPLER**, lade in einer der vier Zeilen mit **LOAD** einen kurzen Mono- oder Stereoclip und stelle **SAMPLER LEVEL** ein. **PLAY** spielt den Clip einmal bis zum Ende. Erneutes Drücken startet denselben Clip von vorn. Verschiedene Plätze können gleichzeitig erklingen. **STOP** beendet einen Platz, **STOP ALL** alle Stimmen und **CLEAR** entfernt die Zuordnung des gewählten Platzes.

Der Sampler geht direkt in den Mastermix. Er ist auch bei geschlossenem Kanalfader hörbar und folgt weder Decktempo noch Crossfader. Im reinen Kopfhörer-Cue hörst du die Decks, im Masteranteil auch den Sampler. Pro Clip gelten höchstens zehn Sekunden und 480.000 Audioframes. Die Abtastrate des Clips darf die des Ausgangs nicht überschreiten. Beim nächsten Start lädt die gespeicherte Bank ohne automatische Wiedergabe.
