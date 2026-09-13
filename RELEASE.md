# Release: Installer bauen, signieren, veröffentlichen

Phase 10 Schritt 11, siehe `DECISIONS.md` ADR-0037 und ADR-0061.

Diese Datei beschreibt, was tatsächlich passiert, wenn ein Tag gesetzt
wird — und **was fehlt**, solange keine Zertifikate hinterlegt sind.
Sie ist bewusst eine Betriebsanleitung, keine Werbung: die
unangenehmen Stellen stehen zuerst.

---

## Der Kurzstand

| | Zustand |
|---|---|
| Installer bauen (Linux/macOS/Windows) | **funktioniert** — `deb`, `AppImage`, `app`, `dmg`, `nsis` |
| Paketmetadaten (Hersteller, Kategorie, Beschreibung, Abhängigkeiten) | **vollständig** |
| Drittanbieter-Lizenzhinweise im Paket | **ja**, `THIRD_PARTY.md` liegt bei |
| SHA-256-Prüfsummen | **ja**, je Plattform eine Datei |
| GitHub-Release auf Tag | **ja**, als Entwurf |
| macOS-Signierung + Notarisierung | **vorbereitet, nie ausgeführt** — kein Apple-Konto verfügbar |
| Windows-Codesigning | **vorbereitet, nie ausgeführt** — kein Zertifikat verfügbar |
| Linux-Signierung | nicht vorgesehen (unüblich für `deb`/`AppImage`) |

„Vorbereitet, nie ausgeführt" heißt: die Schritte stehen in
`.github/workflows/ci.yml`, lesen die unten genannten Secrets und
überspringen sich selbst, wenn diese fehlen. Ob sie mit einem echten
Zertifikat durchlaufen, **ist nicht nachgewiesen** — das ließ sich ohne
Apple-Entwicklerkonto und ohne Windows-Codesigning-Zertifikat nicht
prüfen. Wer den ersten signierten Release macht, sollte mit einem
Testtag anfangen, nicht mit `v1.0.0`.

---

## Release auslösen

```bash
git tag v0.2.0
git push origin v0.2.0
```

Das startet den `release`-Job für alle drei Plattformen. Ergebnis: eine
**Entwurfs**-Release mit den Paketen und Prüfsummen. Entwurf bewusst —
so lässt sich prüfen, was gebaut wurde, bevor es öffentlich ist.

Ohne Tag lässt sich derselbe Job manuell über *Actions →
workflow_dispatch* starten; dann entsteht keine Release, nur
Workflow-Artefakte (14 Tage Aufbewahrung).

---

## Signierung einrichten

### macOS

Nötig: **Apple Developer Program** (kostenpflichtig, jährlich), daraus
ein „Developer ID Application"-Zertifikat.

| Secret | Inhalt |
|---|---|
| `APPLE_CERTIFICATE` | das `.p12`-Zertifikat, base64-kodiert |
| `APPLE_CERTIFICATE_PASSWORD` | dessen Passwort |
| `APPLE_SIGNING_IDENTITY` | z. B. `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID` | Apple-ID für die Notarisierung |
| `APPLE_PASSWORD` | **app-spezifisches** Passwort, nicht das Konto-Passwort |
| `APPLE_TEAM_ID` | Team-Kennung |

```bash
base64 -i zertifikat.p12 | pbcopy   # Inhalt für APPLE_CERTIFICATE
```

Ohne Notarisierung (also nur signiert) zeigt macOS ab Catalina beim
ersten Start weiterhin eine Warnung. Signieren allein genügt also
nicht — `APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` gehören dazu.

`hardenedRuntime` ist in `tauri.conf.json` bereits aktiviert; die
Notarisierung lehnt Pakete ohne sie ab.

### Windows

Nötig: ein **Codesigning-Zertifikat** von einer anerkannten
Zertifizierungsstelle. Seit Juni 2023 verlangen alle CAs Hardware-
Schlüsselspeicher (HSM oder Token) — ein einfaches `.pfx` zum Hochladen
gibt es bei Neuausstellungen nicht mehr. Der vorhandene Schritt geht von
einem `.pfx` aus und funktioniert deshalb nur mit **älteren** oder
intern ausgestellten Zertifikaten.

| Secret | Inhalt |
|---|---|
| `WINDOWS_CERTIFICATE` | das `.pfx`, base64-kodiert |
| `WINDOWS_CERTIFICATE_PASSWORD` | dessen Passwort |

Für ein HSM-gebundenes Zertifikat ist stattdessen
`bundle.windows.signCommand` der richtige Weg (ein eigener Aufruf des
CA-Signierwerkzeugs). Das ist **nicht** eingerichtet, weil sich ohne
konkretes Zertifikat nicht sagen lässt, wie der Aufruf aussehen muss.

### Was ohne Zertifikate passiert

Der Build läuft durch und liefert **unsignierte** Pakete. Auf den
Zielsystemen bedeutet das:

- **macOS**: „… kann nicht geöffnet werden, da der Entwickler nicht
  verifiziert werden kann." Umgehbar über Rechtsklick → Öffnen, aber
  für eine Auslieferung an Fremde unbrauchbar.
- **Windows**: SmartScreen-Warnung „Der Computer wurde durch Windows
  geschützt". Ebenfalls umgehbar, ebenfalls unbrauchbar für Fremde.
- **Linux**: keine Einschränkung, dort ist das der Normalfall.

---

## Laufzeit-Abhängigkeiten

Die Linux-Pakete deklarieren, was die Anwendung wirklich braucht:

- `depends`: `libwebkit2gtk-4.1-0`, `libgtk-3-0`, `librsvg2-2` — ohne
  diese startet sie nicht.
- `recommends`: `ffmpeg`, `libayatana-appindicator3-1`.

**`ffmpeg` ist wichtig und war vorher nirgends deklariert:** die
Video-Funktionen (Import, Vorschau, Schnitt, Export) rufen `ffmpeg` als
externes Programm auf. Fehlt es, startet die Anwendung normal und alle
Foto-Funktionen arbeiten — nur die Video-Seite schlägt fehl. Deshalb
`recommends` und nicht `depends`: ein reiner Foto-Nutzer soll nicht
gezwungen werden, ffmpeg zu installieren.

Auf macOS und Windows gibt es keine Paketabhängigkeiten; dort muss
`ffmpeg` im `PATH` liegen, sonst bleiben die Video-Funktionen stumm.
Ein mitgeliefertes `ffmpeg` (über `bundle.externalBin`) wäre die
robustere Lösung, wirft aber eine eigene Lizenzfrage auf (GPL je nach
Build) und ist deshalb **nicht** gemacht.

---

## Offene Punkte, die eine Entscheidung brauchen

Beides sind keine Programmierfragen, sondern Festlegungen, die nicht
einseitig getroffen werden sollten:

1. **Es gibt keine `LICENSE`-Datei**, obwohl `README.md` das Projekt als
   „fully open sourced" bezeichnet. Ohne Lizenzdatei gilt
   urheberrechtlich „alle Rechte vorbehalten" — der Satz im README
   allein erlaubt niemandem, den Code weiterzugeben oder zu ändern.
   `bundle.license`/`licenseFile` bleiben deshalb leer: eine dort
   eingetragene Lizenz ohne Entscheidung dahinter wäre eine Behauptung.
   Die Lizenz zu wählen ist die einzige offene Aufgabe, die aus
   „open source gemeint" auch „open source wirksam" macht.

2. **LGPL und `rawler` (ADR-0002 Punkt 2).** Die RAW-Dekodierung hängt
   an `rawler` (LGPL-2.1), derzeit statisch gelinkt. Für eine
   **quelloffene** Weitergabe — also das, was der README ankündigt —
   ist das unproblematisch. Nur für eine **geschlossene** verlangt
   LGPL §6, dass Nutzer die Komponente austauschen können; dafür müsste
   `apx-raw` als dynamisch nachladbare Bibliothek gebaut werden. Dieser
   Punkt erledigt sich also mit Entscheidung 1, sobald eine
   Open-Source-Lizenz gewählt ist.

Bis beides geklärt ist, ist ein Release als **Entwurf** genau richtig.
