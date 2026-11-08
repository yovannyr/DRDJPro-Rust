# G01 – Messplan für Updates, Cloud-Sync und Musikdienste

Stand 08.11.2026. Grundlagen: [Spec 1](1-auto-updates/spec.md), [Spec 2](2-cloud-sync/spec.md), [Spec 5](5-streaming-integration/spec.md), [G01-Ersteinstufung](G01_EXTERNAL_FEATURES_AUDIT.md) und ein Entwurf von `deepseek-v4-flash-sovereign`. Die Originalziele bleiben maßgeblich. **Alle 24 Messungen sind offen**: Es gibt noch keinen nachgewiesenen vollständigen Update-, Cloud- oder Musikproviderpfad.

Vor jedem Lauf: Commit und Release-Paket, Betriebssystem, Service-/Clientversion, Geräte, Netzbedingungen, Testdaten und -konten, Zeitquelle, Wiederholungen und Rohwerte protokollieren. Zugänge, Rechte und die technische Nutzung der jeweiligen Dienste sind vorab zu klären; die folgenden Testfälle behaupten keine aktuelle Providerverfügbarkeit. Schutz von Kontodaten, Testdatenisolierung und Fehlerfälle gehören zum Testaufbau. Messungen benötigen echte unterstützte Integrationen; Mocks allein sind kein Provider-Abnahmenachweis.

## Spec 1 – Auto-Updates

| SC | Messstrecke und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Appstart bis Benachrichtigung bei verfügbarem Update: innerhalb 5 Sekunden. Automatischen Startcheck einschließlich spezifizierter Verzögerung separat beobachten. | Verfügbarer Testrelease und Updatepfad. |
| 002 | Nutzer startet Download; Hintergrundübertragung bis Abschluss ohne weiteren Nutzereingriff beobachten. | Downloadpfad und Testrelease. |
| 003 | Repräsentative Erstinstallationsversuche über unterstützte Plattformen; mindestens 95 % Erfolg beim ersten Versuch. Nenner und Fehlversuche dokumentieren. | Signierte Testpakete, Testpopulation und Installation. |
| 004 | Appbeendigung bis Abschluss der Installation: innerhalb 30 Sekunden. | Installationspfad und definierte Zielsysteme. |
| 005 | Nutzerdaten vor und nach Updates einschließlich unterbrochener Fälle vergleichen: kein Datenverlust. | Reproduzierbare Nutzerprofile und Integritätsprüfung. |
| 006 | Während aktiver Downloads die Abstände sichtbarer Fortschrittsupdates messen: mindestens einmal pro Sekunde. | Fortschrittsanzeige und kontrollierte Downloads. |
| 007 | Manuellen Check auslösen bis Ergebnis: innerhalb 2 Sekunden. | Manueller Check und definierte Netzwerkbedingungen. |
| 008 | Offline, Abbruch und Serverfehler auslösen; klare Fehlermeldungen und Weiterarbeit prüfen. | Fehlerbehandlung und kontrollierbare Fehlerfälle. |

## Spec 2 – Cloud-Sync

| SC | Messstrecke und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Ab abgeschlossenem Sync bis eine vollständige Bibliothek auf anderem Gerät zugänglich ist: innerhalb 2 Minuten. Metadaten und gerätespezifische Dateipfade getrennt prüfen; Audiodateisync ist nicht vorgesehen. | Zwei authentifizierte Geräte und lokale Zuordnung der Dateien. |
| 002 | Syncstart bis Abschluss bei Bibliotheken unter 10.000 Tracks: innerhalb 30 Sekunden. | Implementierter Dienst, Testbibliothek und Netzwerkprofil. |
| 003 | Repräsentative Syncvorgänge einschließlich Fehler- und Konfliktfällen; mindestens 99 % erfolgreich ohne Datenverlust. Nenner, Fehlfälle und Datenvergleich dokumentieren. | Dienst, Wiederholungsmenge und Vergleichsdaten. |
| 004 | Vorab definierte Mehrgeräte-Änderungsszenarien ausführen; mindestens 95 % der Nutzeränderungen bleiben erhalten. Konkurrenzen und Löschungen gemäß [D2](G01_DECISIONS_AND_MEASUREMENT.md) getrennt auswerten. | Konfliktszenarien, Nenner und Implementierung. |
| 005 | Änderung des internen Syncstatus bis zur sichtbaren Statusanzeige: innerhalb 1 Sekunde. | Statuspfad und Zeitmessung. |
| 006 | Änderungen offline vornehmen, Verbindung wiederherstellen und vollständigen Sync prüfen. | Persistente Warteschlange und Fehlerwiederholung. |
| 007 | Mindestens 100.000 Tracks synchronisieren und bedienen; Performance ohne Degradation prüfen. Rohwerte und Vergleichsbasis festlegen. | Testdaten und vorab geklärte Definition von Degradation. |
| 008 | Automatischen Sync unter realer Nutzung beobachten; keine UI-Unterbrechung und keine bewusste Nutzerintervention gemäß Spec. | Hintergrundpfad und geeigneter Nutzungs-/UI-Test. |

## Spec 5 – Streaming-Integration

| SC | Messstrecke und Originalziel | Offene Voraussetzung |
|---|---|---|
| 001 | Pro gefordertem Dienst Authentifizierungsstart bis erfolgreicher Verbindung: unter 30 Sekunden. | Erlaubter Zugang, Konten und Provideranbindung. |
| 002 | Suchabsendung bis sichtbare Ergebnisse: innerhalb 2 Sekunden; Dienste und Netzbedingungen getrennt ausweisen. | Tatsächlicher Suchpfad und Dienste. |
| 003 | Ladeaktion bis zur startenden Deckwiedergabe: innerhalb 3 Sekunden. | Lizenzierte, verfügbare Tracks und echter Audiopfad. |
| 004 | Repräsentative Suchmenge vorab festlegen; mindestens 95 % der gesuchten Tracks verfügbar und spielbar. Region, Konto, Nenner und fehlgeschlagene Tracks berichten. | Geeignete Providerzugänge und definierte Stichprobe. |
| 005 | Tatsächliche Qualität der höchsten verfügbaren Stufe je Dienst und Konto messen; Originalangaben der Spec einschließlich Spotify 320 kbps+ und TIDAL-Lossless prüfen. | Berechtigungen, Qualitätsnachweis und aktuelle technische/vertragliche Machbarkeit. |
| 006 | Persönliche Playlist laden bis zur Darstellung: innerhalb 5 Sekunden unabhängig von Playlistgröße. Unterschiedliche Größen und Paginierung prüfen. | Provideranbindung und Testplaylists unterschiedlicher Größe. |
| 007 | Tokenablauf unter laufender Nutzung auslösen; automatische Erneuerung ohne Nutzerunterbrechung prüfen. | Authentifizierungs- und Refreshpfad. |
| 008 | Rate-Limits, nicht verfügbare Tracks, Netzwerk- und Authentifizierungsfehler auslösen; handlungsfähige Meldungen und Weiterarbeit prüfen. | Implementierte Fehlerpfade und kontrollierbare Szenarien. |

Wo eine Spec keine Stichprobengröße, genaue Nutzerwahrnehmung oder Definition von Performance-Degradation festlegt, muss das Messprotokoll diese vorab konkretisieren; keine zusätzliche vom Modell vorgeschlagene Zahl wird hier als Originalanforderung ausgegeben.
