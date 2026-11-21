# Controller und Anschlüsse

Für das Abspielen genügt ein Stereoausgang. Möchtest du einen Titel im Kopfhörer vorbereiten, während das Publikum den anderen hört, braucht DrDJ Pro vier separat ansprechbare Ausgangskanäle am **selben Audiogerät**. Ein Controller kann gleichzeitig Audio ausgeben und über MIDI die Decks steuern. Gerätauswahl und MIDI-Verbindung haben eigene Statusanzeigen.

## Wo finde ich die Einstellungen?

| Aufgabe | Aktueller Entwicklungsstand mit Zahnrad | Desktop Preview RC1 |
| --- | --- | --- |
| Audiogerät oder Master-Ausgangspaar wechseln | Zahnrad rechts oben → **Audio** | **AUDIO** in der Mixerleiste, Ausgangspaar oben neben dem Programmnamen |
| Vorhören und Split Cue einrichten | **MON** in der Mixerleiste oder Zahnrad → **Mixer** | **MON** in der Mixerleiste |
| MC4000 neu verbinden | Zahnrad → **Controller → RECONNECT MIDI** | **RECONNECT MIDI** in der unteren Leiste |
| USB-Geräte ansehen oder HID-Profil öffnen | Zahnrad → **Controller** | **USB / HID** in der unteren Leiste |
| Separates Display und Standard-MIDI-Pads öffnen | Zahnrad → **Library & Ansicht → REMOTE** | **REMOTE** in der unteren Leiste |

Die Zahnrad-Navigation gilt für **Classic** und **Modern**. Mit `Esc` oder **Zurück zu den Decks** schließt du sie. In RC1 sind die oben genannten Direktzugriffe in beiden Layouts verfügbar.

## Master und Kopfhörer anschließen

1. Schließe Lautsprecher oder Verstärker an den Master-Ausgang des Interfaces bzw. Controllers an. Stecke den Kopfhörer in den zugehörigen Kopfhörerausgang. Beginne mit niedriger Lautstärke am Gerät.
2. Öffne die Audio-Einstellungen und klicke **REFRESH**. Wähle das Gerät aus. Pausiere beide Decks und drücke **APPLY NOW**. Prüfe unter **CURRENT**, welches Gerät nun geöffnet ist. **SAVE FOR NEXT START** speichert die Auswahl für den nächsten Start.
3. Stelle den **Master-Ausgang** für diese Belegung auf **OUT 1/2**. Das Kopfhörersignal liegt auf **OUT 3/4**. Liefert das Gerät dem Programm nur zwei Kanäle, ist getrenntes Stereo-Vorhören über **MON** nicht verfügbar. Auch ein zweites, separat angeschlossenes Stereogerät ergibt hier kein gemeinsames Vierkanalgerät.
4. Starte Titel A auf dem Master. Ziehe den Kanalfader B zu. Wähle unter **MON** die Option **CUE B (pre-fader)** und starte Titel B. Drehe **HEADPHONE LEVEL** vorsichtig auf und den Mix-Regler in Richtung **CUE**. Im Kopfhörer solltest du B hören, während am Master weiterhin A spielt.
5. Bewege den Mix-Regler in Richtung **MASTER**, um A dazuzumischen. Bei **SPLIT CUE** liegt das Vorhörsignal links in Mono und der Master rechts in Mono. In der Mitte sind beide Seiten hörbar, an den Endanschlägen jeweils nur eine. **HEADPHONE LEVEL** regelt beide Seiten. Nimm für diesen Test Mono- oder Stereotitel, da Mehrkanaltitel die zusätzlichen Ausgänge selbst belegen können.

## Den Denon DJ MC4000 verbinden

Schließe den MC4000 über USB an und starte das Programm im normalen Audiobetrieb. Es sucht den MIDI-Port **MC4000** beim Start und regelmäßig während des Betriebs. **MC4000 input + LEDs connected** bestätigt den Eingang und die Rückmeldung an die LEDs. **MC4000 input connected; LEDs unavailable (retrying)** bedeutet, dass die Bedieneingaben bereits funktionieren können, während das Programm weiter nach dem LED-Ausgang sucht.

Bewege zur Kontrolle einen Kanalfader und beobachte den entsprechenden Regler auf dem Bildschirm. Öffne bei fehlender Reaktion **RECONNECT MIDI**. Nach einer erkannten Trennung ist diese Schaltfläche im aktuellen Entwicklungsstand zusätzlich in der Live-Ansicht erreichbar. Bei der Meldung über mehrere gleichnamige Ports muss der MIDI-Eingang **MC4000** vom Betriebssystem eindeutig angeboten werden. Ob der Controller auch den Ton ausgibt, erkennst du separat in den Audio-Einstellungen unter **CURRENT**.

| Am MC4000 | Bedienung im Set |
| --- | --- |
| Browse-Encoder, **BACK**, **LOAD** | Drehen wählt einen Titel. Mit **SHIFT** bewegt sich die Auswahl in Zehnerschritten. Ein Druck auf den Encoder wechselt zwischen Titeln und Crates. **BACK** führt zur Crate-Auswahl. **LOAD** lädt auf das gewählte pausierte Deck. |
| Zweimal **LOAD** am selben Zieldeck | Zwei kurze Betätigungen innerhalb von 350 ms übernehmen den Titel des anderen Decks inklusive Position, Tempo und Wiedergabestatus. Dazu muss das Zieldeck pausiert sein. Beachte, dass der erste Druck sofort als normaler Ladebefehl wirkt. |
| **CUE**, **SHIFT + CUE**, **SHIFT + PLAY** | **CUE** setzt den temporären Punkt oder hört ihn bei gedrückter Taste vor. **SHIFT + CUE** springt zum Titelanfang. **SHIFT + PLAY** startet am temporären Punkt erneut. |
| Jogwheel, **VINYL**, **SHIFT** | Mit **VINYL** kannst du die berührte Scheibe zum Scratchen drehen. Loslassen setzt den vorherigen Spiel- oder Pausenzustand fort. **SHIFT** und Drehen suchen schneller durch den Titel. **SHIFT + VINYL** spielt bei laufendem Titel vorübergehend rückwärts. Beim Loslassen hörst du wieder die inzwischen weitergelaufene normale Zeitposition. |
| Pitch-Fader und **SYNC** | Nach einem Tempoeingriff am Bildschirm nimmt der Hardwarefader den Wert wieder auf, sobald du ihn an die aktuelle Position führst. So vermeidest du einen Temposprung. **SYNC** lässt dieses Deck dem anderen folgen, **SHIFT + SYNC** löst die Bindung. |
| Loop-Tasten | **IN** und **OUT** setzen die Grenzen. **AUTO** schaltet den Auto-Loop. Halbieren und Verdoppeln ändern seine Länge. **SHIFT + AUTO** legt vier musikalische Takte an. Bei aktivem Loop wählen die Hardwaretasten **IN** bzw. **OUT** die Grenze zur Feinkorrektur per Jogwheel. |
| Vier Deck-Pads | Im Modus **MC: HOT CUES** setzen oder spielen die Pads Hot Cues 1–4 dieses Decks. Mit **SHIFT** löschst du den jeweiligen Punkt. Im Modus **MC: SAMPLER** starten dieselben Pads Samples 1–4. Mit **SHIFT** stoppst du den jeweiligen Clip. Beide Decks greifen auf dieselbe Samplebank zu. Der Padmodus lässt sich je Deck über die entsprechende Schaltfläche in Classic wechseln. |
| FX-Tasten und Drehregler | Die drei FX-Tasten schalten die drei Effektplätze des Decks. In **FX** ist **Legacy knobs: Wet / Feedback / Time** standardmäßig aktiv. Die drei Knöpfe bedienen damit die Delay-Parameter des ersten Platzes. Schalte die Option aus, wenn jeder Knopf die Intensität seines eigenen Effektplatzes regeln soll. |

## Weitere USB- und HID-Geräte

Öffne den **USB DEVICE FINDER** und drücke **REFRESH**. Wähle das Gerät anhand der Hersteller- und Produktkennung (VID/PID) sowie der Busadresse. **READ DEVICE STRINGS** fragt die verfügbaren Zusatzangaben ab. Ein Eintrag in dieser Liste bestätigt die Erkennung. Ob du mit dem Gerät Deckfunktionen auslösen kannst, hängt von der passenden MIDI-Anbindung oder einem HID-Profil ab.

Für ein HID-Bediengerät mit passendem Profil öffne **HID**, wähle dessen Eintrag und lade das zum Gerät gehörende JSON-Profil mit **LOAD MAPPING**. Prüfe die angezeigte Profilbezeichnung und drücke **ENABLE**. Lasse zunächst alle Tasten los und betätige dann die zugeordnete Funktion. Das Programm akzeptiert jeweils ein aktives HID-Gerät und sperrt Systemtastaturen und Mäuse in dieser Auswahl. **STOP** beendet die HID-Steuerung. Über **USB LIST** kehrst du zur Übersicht zurück.

**Timecode-Steuerung (DVS).** Nur eine eigens mit DVS-Unterstützung erstellte Ausgabe zeigt **DVS AUDIO**. Die regulären RC1-Installationspakete enthalten diese Option nicht. Ist sie vorhanden, wähle den Stereo-Eingang und die passende Timecode-Edition. Stelle bei Bedarf **Low-level phono signal** und den Kalibrierversatz ein. Mit **ARM A** oder **ARM B** übergibst du das betreffende geladene Deck an die Timecode-Steuerung. Nach Signalverlust oder manuellem Transporteingriff musst du erneut **ARM** drücken. **STOP A/B** beendet die Steuerung am jeweiligen Deck.

## Zweite Anzeige und zusätzliche MIDI-Pads

**REMOTE** öffnet ein separates Programmfenster. Ziehe es bei Bedarf auf einen zweiten Bildschirm, wähle **Deck A** oder **Deck B** und blende etwa Titel, Loop, Cues, Pegel, EQ oder Crossfader ein. Zur Wahl stehen **128 × 64** und **128 × 32** Bildpunkte. Bei der kleinen Auflösung wechselt **Transport page (32px)** zur Transportseite. Schließen lässt die laufenden Decks weiterspielen.

Für einen Standard-MIDI-Padcontroller drücke in diesem Fenster **REFRESH**, wähle den Eingangsport und stelle **Channel** und **First note** passend zum Gerät ein. **OPEN PADS** aktiviert 16 aufeinanderfolgende Noten. Die ersten acht bedienen die Hot Cues von A, die folgenden acht die von B. Bei **First note 36** und Kanal 1 steuern die Noten 36–43 Deck A und die Noten 44–51 Deck B. Die Anschlagstärke erscheint in der Statusanzeige. Der Deckpegel wird am Mixer eingestellt. Nach Änderungen an Kanal, erster Note oder Anschlagkurve drücke erneut **OPEN PADS**. **STOP PADS** gibt den Port frei.

## Wenn etwas stumm bleibt

| Beobachtung | Was du prüfen kannst |
| --- | --- |
| Der Bildschirm reagiert auf den Controller, die Lautsprecher bleiben stumm | Kontrolliere **CURRENT**, Master-Ausgangspaar, laufendes Deck, Kanalfader, Crossfader und den physischen Masterregler. Die MIDI-Statusmeldung beschreibt nur die Steuerung. |
| Der Master spielt, im Kopfhörer fehlt das zweite Deck | Kontrolliere vier verfügbare Kanäle, Master auf **OUT 1/2**, **CUE A/B**, Kopfhörer-Level und die Stellung des Cue/Master-Mixreglers. In **SPLIT CUE** kann ein Endanschlag eine Seite vollständig ausblenden. |
| Nach dem Abziehen des Audiogeräts erscheint eine Ausgabewarnung | Schließe dasselbe Gerät wieder an und wähle **REOPEN CURRENT**. Soll ein anderes Gerät übernehmen, pausiere beide Decks, wähle es unter **REFRESH** aus und drücke **APPLY NOW**. Kontrolliere nach dem Wechsel die Ausgangspaare, bevor du die Wiedergabe wieder startest. |
| Ein Pad löst die falsche Aktion aus | Beim MC4000 prüfe den Padmodus des betreffenden Decks. Bei Standard-MIDI-Pads prüfe im **REMOTE**-Fenster den Kanal und die erste Note und öffne den Eingang erneut. |

Für das Setzen von Cue-Punkten, den ersten Übergang und die Bedienung des Samplers siehe [Decks und Mixer](Decks-und-Mixer.md).
