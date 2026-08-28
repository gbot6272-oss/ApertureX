# Claude Code Prompt — RAW-Editor "Aperture X" (Lightroom-Klon++)

> Alles ab hier in Claude Code einfügen. Vor dem ersten Einfügen: leeres Repo anlegen, `cd` hinein, `claude` starten.

---

## 0. Rolle und Auftrag

Du bist Lead-Engineer für ein Desktop-Programm namens **Aperture X**: ein nicht-destruktiver RAW-Foto-Editor und Katalog-Manager, der Adobe Lightroom Classic funktional vollständig nachbaut und darüber hinaus deutlich mehr kann.

Du arbeitest **nicht** in einem Rutsch. Du arbeitest in Phasen, jede Phase endet mit lauffähigem, getestetem Code und einem Commit. Nach jeder Phase stoppst du und berichtest kurz, was fertig ist und was als Nächstes kommt.

Bevor du die erste Zeile Code schreibst:
1. Lege `PLAN.md` an mit der vollständigen Phasenliste, Aufgabenpunkten und Abhängigkeiten.
2. Lege `ARCHITECTURE.md` an mit Modulgrenzen, Datenfluss und Begründung der Technologiewahl.
3. Lege `DECISIONS.md` an (ADR-Format, ein Eintrag pro Architekturentscheidung).
4. Lege `FEATURES.md` an — die komplette Feature-Matrix aus diesem Prompt, jeder Punkt mit Checkbox und Status.
5. Zeige mir diese vier Dateien und warte auf mein "go", bevor Phase 1 startet.

---

## 1. Technologie-Stack (verbindlich, Abweichung nur mit ADR-Begründung)

| Schicht | Technologie |
|---|---|
| Shell / Fenster | Tauri 2 (Rust-Backend, WebView-Frontend) |
| Kern-Bildpipeline | Rust, `no_std`-nah, GPU-beschleunigt |
| GPU | `wgpu` (Vulkan/Metal/DX12), WGSL-Compute-Shader |
| RAW-Dekodierung | `rawler` / `libraw`-Bindings, DNG-Support nativ |
| Farbmanagement | `lcms2` (LittleCMS), ICC-Profile, OCIO-kompatibel |
| Metadaten | `rexiv2` / eigener EXIF-XMP-IPTC-Parser |
| Katalog-DB | SQLite (WAL-Modus) + `sqlx`, Migrationen versioniert |
| Frontend | React 19 + TypeScript, Vite |
| State | Zustand + Immer, mit Undo/Redo-Middleware |
| Styling | Tailwind CSS 4, eigenes Dark-First-Theme |
| Canvas/Viewer | WebGL2 bzw. WebGPU, kein DOM-Rendering für Bildbereich |
| Tests | `cargo test`, Vitest, Playwright für E2E |
| CI | GitHub Actions: Build für Windows, macOS, Linux |

**Nicht verhandelbar:** Die gesamte Bildverarbeitung läuft in Rust auf der GPU. Das Frontend rendert nur UI und schickt Edit-Parameter. Kein Bildpixel wird in JavaScript verarbeitet.

---

## 2. Kernarchitektur

### 2.1 Non-destruktive Edit-Pipeline
- Jedes Foto = **Originaldatei (read-only)** + **Edit Decision List (EDL)** als JSON/CBOR.
- Die EDL ist ein gerichteter azyklischer Graph aus Operationen. Reihenfolge ist definiert und dokumentiert:
  1. RAW-Dekodierung → 2. Demosaicing → 3. Linearisierung → 4. Weißabgleich → 5. Objektivkorrektur → 6. Geometrie/Crop → 7. Belichtung/Ton → 8. Farbe/HSL → 9. Lokale Masken → 10. Details/Rauschen → 11. Effekte → 12. Output-Transform/Soft-Proof.
- Jede Operation ist ein eigenes Rust-Modul mit eigenem Shader, eigenem Test und eigener Serialisierung.
- Versionierte EDL: Schema-Migration muss alte Kataloge öffnen können.

### 2.2 Rendering
- **Zwei Auflösungen:** Proxy (Bildschirmgröße, interaktiv, < 16 ms pro Regler-Änderung) und Full (Export, volle Auflösung, gekachelt).
- Tile-basiertes Rendering mit Cache-Invalidierung ab der geänderten Pipeline-Stufe — nicht die ganze Kette neu rechnen.
- Interne Verarbeitung in 32-bit Float, linearer Farbraum, ProPhoto-Primärvalenzen.
- GPU-Fallback auf CPU (Rayon, SIMD) muss existieren und getestet sein.

### 2.3 Katalog
- SQLite mit Tabellen: `photos`, `folders`, `collections`, `collection_sets`, `smart_collections`, `keywords`, `keyword_tree`, `edits`, `snapshots`, `history`, `presets`, `people`, `faces`, `stacks`, `virtual_copies`, `publish_services`, `metadata_presets`.
- Volltextsuche via FTS5 über Dateiname, Keywords, Titel, Caption, Kameramodell, Objektiv.
- Sidecar-Export: `.xmp` schreiben/lesen, kompatibel zu Adobe.
- Katalog-Backup, -Optimierung, -Reparatur, Zusammenführen zweier Kataloge.

### 2.4 Performance-Ziele (harte Akzeptanzkriterien)
- Regler-Bewegung → sichtbares Update: **< 16 ms** bei 24 MP Proxy.
- Bildwechsel in der Entwickeln-Ansicht: **< 200 ms**.
- Import 1000 RAWs inkl. Vorschau-Generierung: **< 4 Minuten** auf moderner Hardware.
- Bibliotheks-Raster mit 100.000 Bildern: flüssiges Scrollen, virtualisiert.
- Speicherverbrauch im Leerlauf: < 800 MB.

---

## 3. Module und Feature-Umfang

### 3.1 Modul BIBLIOTHEK
Import mit Kopieren/Verschieben/Hinzufügen/DNG-Konvertierung · Import-Presets · automatisches Umbenennen mit Token-System (Datum, Sequenz, Kamera, Custom Text, Metadatenfelder) · Duplikaterkennung per Hash + Perceptual Hash · Ordnerbaum mit Synchronisation · Ordner fehlend/wiederfinden · Sammlungen, Sammlungssätze, intelligente Sammlungen mit verschachtelten UND/ODER-Regeln · Zielsammlung · Stapel (automatisch nach Zeit, manuell) · virtuelle Kopien · Bewertung 0–5 · Farbmarkierungen (erweiterbar auf beliebig viele) · Flaggen · Schlagworthierarchie mit Synonymen, Export-Steuerung, Auto-Vervollständigung · Schlagwortvorschläge · Metadaten-Presets · Stapel-Metadatenbearbeitung · EXIF/IPTC/XMP-Editor mit allen Feldern · Rasteransicht, Lupe, Vergleich, Übersicht, Personenansicht · Filterleiste (Text, Attribut, Metadaten, kombiniert) · Filter-Presets · Sortierung nach beliebigem Feld · Schnellentwicklung im Raster · Vorschau-Cache-Verwaltung (Standard, 1:1, Smart Previews) · Offline-Bearbeitung über Smart Previews · Sekundäres Display mit unabhängiger Ansicht.

**Über Lightroom hinaus:** frei definierbare Metadaten-Felder · beliebig viele Farbmarkierungen mit eigenen Namen · Tag-Regeln (wenn Kamera = X und Blende < 2.0 → Tag "Portrait") · Katalog-Statistiken-Dashboard (Objektivnutzung, Brennweitenverteilung, Aufnahmezeiten-Heatmap, ISO-Histogramm über Jahre) · Duplikat-Assistent mit Seite-an-Seite-Vergleich und Auto-Auswahl der besten Version nach Schärfe/Augen-offen.

### 3.2 Modul ENTWICKELN — Globale Werkzeuge
**Grundeinstellungen:** Weißabgleich (Temperatur, Tint, Pipette, Presets pro Kamera) · Belichtung · Kontrast · Lichter · Tiefen · Weiß · Schwarz · Textur · Klarheit · Dunst entfernen · Dynamik · Sättigung.

**Gradationskurve:** Punktkurve und parametrische Kurve · RGB-Verbundkurve plus einzelne R/G/B-Kanäle · Luminanz-Kurve · frei setzbare Punkte mit numerischer Eingabe · Kurven-Presets.

**HSL / Farbe:** Farbton, Sättigung, Luminanz für 8 Standardbereiche.

**Farbmischer erweitert:** frei definierbare Farbbereiche statt fester acht — Nutzer klickt eine Farbe im Bild, bekommt einen eigenen Regler-Satz mit einstellbarer Bandbreite und Weichzeichnung des Übergangs.

**Farbklassifizierung / Color Grading:** Farbräder für Schatten, Mitteltöne, Lichter, Global · Luminanz und Mischung pro Rad · Balance · Überblendung.

**Details:** Schärfung (Betrag, Radius, Detail, Maskierung) · Luminanzrauschen (Betrag, Detail, Kontrast) · Farbrauschen (Betrag, Detail, Glättung) · Deconvolution-Schärfung als Alternative.

**Objektivkorrekturen:** Profilbasiert (Datenbank mit Profilen, eigene Profile importierbar) · Chromatische Aberration automatisch und manuell · Vignettierung · Verzeichnung · Perspektive/Upright (Auto, Level, Vertical, Full, Guided mit bis zu 4 Hilfslinien) · manuelle Transformation (Vertikal, Horizontal, Drehen, Seitenverhältnis, Skalieren, X/Y-Versatz).

**Effekte:** Nachträgliche Vignettierung (Betrag, Mittelpunkt, Rundheit, weiche Kante, Lichter) · Körnung (Betrag, Größe, Unregelmäßigkeit).

**Kalibrierung:** Prozessversion · Schattentönung · Primärfarben-Regler R/G/B jeweils Farbton und Sättigung · Kamera-Profile (Adobe Standard, Kamera-Emulationen, DCP-Import).

**Geometrie:** Freistellen mit Seitenverhältnis-Presets, eigenen Verhältnissen, Rasterüberlagerungen (Drittel, Goldener Schnitt, Diagonalen, Spirale, Dreiecke), Winkel-Werkzeug, Auto-Ausrichtung am Horizont.

**Reparatur:** Bereichsreparatur klonen/reparieren mit variabler Deckkraft, Größe, weicher Kante · Auto-Quellenfindung · Visualisierung von Sensorflecken · Inhaltsbasiertes Füllen für größere Bereiche.

### 3.3 Modul ENTWICKELN — Lokale Anpassungen
Maskensystem mit vollem Ebenenmodell:
- Maskentypen: Pinsel · Linearer Verlauf · Radialer Verlauf · Farbbereich · Luminanzbereich · Tiefenbereich (falls Tiefendaten vorhanden) · **KI-Motiv** · **KI-Himmel** · **KI-Hintergrund** · **KI-Objekte** (Segmentierung per Klick) · **KI-Personen** mit Unterteilung in Gesichtshaut, Körperhaut, Augen (Sklera/Iris), Augenbrauen, Lippen, Zähne, Haare, Kleidung.
- Masken kombinieren mit Hinzufügen, Subtrahieren, Schneiden.
- Pro Maske: alle globalen Regler verfügbar plus Deckkraft, Weichzeichnung, Umkehren, Bereich verfeinern.
- Maskengruppen, Umbenennen, Ein-/Ausblenden, Maskenüberlagerung in wählbarer Farbe, Duplizieren, auf anderes Foto übertragen.
- **Über Lightroom hinaus:** echte Ebenen-Mischmodi pro Maske (Multiplizieren, Weiches Licht, Farbe, Luminanz …) · Masken als wiederverwendbare Bausteine speichern · Maskenkette mit Sortierung per Drag & Drop.

### 3.4 Modul ENTWICKELN — Workflow
Verlauf mit unbegrenzten Schritten, klickbar, Schritte benennbar · Schnappschüsse · Vorher/Nachher in vier Ansichten (links/rechts, geteilt, oben/unten, geteilt vertikal) · Einstellungen kopieren/einfügen mit granularer Auswahl · Vorherige übernehmen · Synchronisieren über beliebig viele Bilder · Auto-Sync-Modus · Referenzansicht (Referenzbild links, Arbeitsbild rechts) · Soft-Proof mit Zielprofil, Renderpriorität, Farbumfangswarnung, Papierweiß-Simulation.

### 3.5 PRESET- UND TEMPLATE-SYSTEM (Kernanforderung)
Dies ist das wichtigste Alleinstellungsmerkmal. Baue es als eigenes, tief integriertes Subsystem.

**Preset-Grundlagen**
- Presets speichern eine wählbare Teilmenge der EDL. Beim Speichern zeigt ein Dialog jede einzelne Einstellungsgruppe mit Checkbox.
- Ordnerhierarchie beliebiger Tiefe, Drag & Drop, Favoriten, Suche, Tags.
- Preset-Stärke: ein Regler 0–200 % skaliert alle enthaltenen Werte beim Anwenden — auch nachträglich änderbar, solange kein anderer Edit dazwischen liegt.
- Live-Vorschau beim Überfahren mit der Maus im Bild und in der Preset-Liste als Thumbnail des aktuellen Bildes.
- Preset-Stapel: mehrere Presets nacheinander anwenden, Reihenfolge editierbar.
- Bedingte Presets: "wenn ISO > 3200, wende zusätzlich Rauschprofil an", "wenn Objektiv = 35 mm, Vignette -10". Regeln über eine kleine Bedingungssprache im UI-Builder.
- Import/Export: eigenes `.apx`-Format **und** vollständige Kompatibilität mit Adobe `.xmp` und `.lrtemplate` in beide Richtungen.
- Versionierung: Preset ändern erzeugt neue Version, alte bleibt erhalten, Diff-Ansicht zwischen Versionen.

**Preset-Generator (KI)**
- Nutzer beschreibt in natürlicher Sprache: "warmer Filmlook, angehobene Schwarztöne, entsättigtes Grün, leichte Körnung" → System generiert ein vollständiges Preset.
- Implementierung: Anfrage an ein LLM (Anthropic API, Modell `claude-sonnet-4-6`, API-Key aus den Einstellungen des Nutzers, niemals im Repo) mit einem strikten JSON-Schema aller Parameter. Antwort wird gegen das Schema validiert, außerhalb der Wertebereiche wird geklemmt, ungültige Antworten werden bis zu dreimal neu angefragt.
- **Referenzbild-Modus:** Nutzer lädt ein Bild mit gewünschtem Look → System analysiert Histogramm, Farbverteilung in LAB, Tonwertkurve, Sättigungsprofil und berechnet per Optimierung (Gradientenverfahren über die Pipeline-Parameter) ein Preset, das das aktuelle Bild an den Referenz-Look annähert. Kein LLM nötig, rein numerisch.
- **Variationen-Generator:** aus einem Preset zehn Abwandlungen erzeugen (wärmer, kühler, kontrastreicher, flacher, entsättigt, …), als Kontaktbogen zur Auswahl.
- **Preset aus Bearbeitung lernen:** Nutzer bearbeitet 20 Bilder ähnlich → System erkennt wiederkehrende Muster und schlägt ein Preset vor.

**Templates (über Presets hinaus)**
- **Export-Templates:** Zielordner, Dateiformat, Qualität, Farbraum, Größenbegrenzung, Schärfung für Medium, Metadaten-Umfang, Wasserzeichen, Nachbearbeitungs-Aktion. Mehrere Ziele gleichzeitig in einem Durchgang.
- **Wasserzeichen-Templates:** Text und Grafik, Position, Deckkraft, Größe, Versatz, Schlagschatten.
- **Metadaten-Templates:** Copyright, Ersteller, Kontakt, IPTC-Vollsatz.
- **Import-Templates**, **Umbenennungs-Templates** mit Token-Editor.
- **Layout-Templates** für Druck, Buch, Diashow, Web.
- **Workflow-Templates:** komplette Pipeline aus Import → Filter → Preset → Export als ein Klick.
- **Template-Marktplatz-Struktur:** lokales Repository-Format, Manifest mit Autor, Lizenz, Vorschaubildern, Versionsnummer, Abhängigkeiten. Installation aus einer Datei oder einem Ordner.

### 3.6 Weitere Module
**Karte:** GPS aus EXIF, Karten-Ansicht, Tracklog-Import (GPX), Fotos per Drag auf Karte setzen, Ortsschlagworte automatisch (Reverse Geocoding), Reiserouten-Ansicht.

**Buch:** Seitenlayouts, Vorlagen, Text-Stile, automatische Befüllung, PDF-Export, Druckerei-Presets.

**Diashow:** Übergänge, Ken-Burns-Effekt, Musik-Synchronisation, Intro/Outro-Screens, Video-Export (MP4 über ffmpeg).

**Drucken:** Einzelbild, Kontaktbogen, Bilderpaket, benutzerdefiniertes Raster, Randeinstellungen, Zellen, Zoom, Druckschärfung, Farbmanagement, Druckauflösung, Speichern als JPEG.

**Web:** HTML- und responsive Galerie-Generator, Themes, Upload via FTP/SFTP.

**Export:** Formate JPEG, PNG, TIFF, PSD, DNG, WebP, AVIF, HEIF, JPEG XL · Farbräume sRGB, AdobeRGB, ProPhoto, Display-P3, benutzerdefinierte ICC · Bit-Tiefe 8/16 · Größenbegrenzung nach Kante, Megapixeln, Dateigröße · Ausgabeschärfung nach Medium · Wasserzeichen · Metadaten-Filter · Warteschlange mit Fortschritt, Pausieren, Priorisieren.

**Zusätzliche Module, die Lightroom nicht hat:**
- **Node-Editor:** die gesamte Pipeline als Knotengraph sichtbar und umbaubar. Reihenfolge der Operationen frei ändern, Verzweigungen, Mischknoten. Für Fortgeschrittene, umschaltbar.
- **Stapelverarbeitungs-Konsole:** Regeln auf Tausende Bilder anwenden, mit Vorschau der betroffenen Menge, Trockenlauf, Rückgängig-Machen der gesamten Aktion.
- **Fokus-Stacking**, **HDR-Zusammenführung**, **Panorama-Zusammenführung** (sphärisch, zylindrisch, perspektivisch, mit Auto-Crop und Kantenfüllung), **Astro-Stacking** mit Sternausrichtung.
- **Tethered Shooting** über gphoto2/PTP mit Live-View und Auto-Preset beim Import.
- **Vergleichs-Grid:** bis zu neun Versionen desselben Bildes nebeneinander, synchronisierter Zoom.
- **Skript-API:** Lua oder Rhai für Nutzer-Automatisierung, plus Plugin-System mit stabilem ABI.
- **Zeitleisten-Ansicht:** alle Bearbeitungen eines Fotos über die Zeit als Zeitstrahl.
- **Verlaufs-Vergleich:** zwei beliebige Verlaufsschritte gegenüberstellen.
- **Kollaborationsmodus:** Katalog-Teilfreigabe als Datei, Merge von Bearbeitungen zweier Personen mit Konfliktauflösung.
- **Barrierefreiheit:** vollständige Tastaturbedienung, Screenreader-Labels, Kontrastmodus, skalierbare UI 75–200 %.

---

## 4. UI-Anforderungen
- Dark-First, aber vollständiges helles Theme und benutzerdefinierte Themes über Design-Tokens.
- Layout: linke Palette (Navigator, Presets, Sammlungen), Zentrum (Bild), rechte Palette (Werkzeuge), unten Filmstreifen, oben Modul-Umschalter. Alle Paletten ein-/ausklappbar, in der Breite ziehbar, Anordnung speicherbar als **Arbeitsbereich-Preset**.
- Jeder Regler: Doppelklick setzt zurück, numerische Direkteingabe, Pfeiltasten für Feinschritte, Shift für große Schritte, Alt für Maskenvorschau wo sinnvoll.
- Vollständig belegbare Tastenkürzel, Cheatsheet-Overlay auf `?`.
- Befehlspalette auf `Strg/Cmd+K`, die jede Funktion und jedes Preset findet.
- Kein Blockieren der UI: jede lange Operation läuft im Hintergrund mit sichtbarem Fortschritt und Abbruchmöglichkeit.

---

## 5. Phasenplan

**Phase 1 — Fundament.** Repo-Struktur, Tauri-Shell, Rust-Workspace, SQLite-Katalog mit Migrationen, RAW-Dekodierung, Anzeige eines Bildes, Zoom/Pan. Ergebnis: Bild öffnen und ansehen.

**Phase 2 — Pipeline-Kern.** wgpu-Setup, Shader-Framework, Farbmanagement, EDL-Datenmodell, die Grundeinstellungs-Regler (WB, Belichtung, Kontrast, Lichter, Tiefen, Weiß, Schwarz), Tile-Cache, Verlauf mit Undo/Redo. Ergebnis: interaktives Entwickeln.

**Phase 3 — Bibliothek.** Import, Ordner, Raster, Filmstreifen, Vorschau-Generierung, Bewertungen/Flaggen/Farben, Sammlungen, Filter, Metadaten-Panel, FTS-Suche.

**Phase 4 — Entwickeln vollständig.** Kurven, HSL, Farbmischer, Color Grading, Details, Objektivkorrekturen, Effekte, Kalibrierung, Crop/Geometrie, Reparatur.

**Phase 5 — Preset- und Template-System.** Komplett wie in 3.5, inklusive Adobe-Import/Export, Preset-Stärke, bedingten Presets, Live-Vorschau.

**Phase 6 — Masken und lokale Anpassungen.** Pinsel, Verläufe, Bereichsmasken, Maskenkombination, Ebenen-Mischmodi.

**Phase 7 — KI-Funktionen.** Motiv-/Himmel-/Personen-Segmentierung (ONNX-Runtime, Modelle lokal), Preset-Generator per LLM, Referenzbild-Matching, Auto-Tagging.

**Phase 8 — Export und Ausgabe-Module.** Export-Engine, Warteschlange, Wasserzeichen, dann Drucken, Diashow, Buch, Web, Karte.

**Phase 9 — Fortgeschrittenes.** Node-Editor, Panorama/HDR/Fokus-Stacking, Tethering, Skript-API, Plugin-System.

**Phase 10 — Politur.** Performance-Profiling gegen die Ziele aus 2.4, Barrierefreiheit, Lokalisierung (Deutsch und Englisch), Onboarding, Installer und Signierung für alle drei Plattformen.

---

## 6. Arbeitsweise (verbindlich)

- **Ein Feature = ein Branch = ein Commit-Set = ein Test.** Conventional Commits.
- Vor jeder Phase: kurzer Plan in `PLAN.md` aktualisieren. Nach jeder Phase: Status in `FEATURES.md` abhaken.
- **Keine Platzhalter, kein `todo!()`, kein auskommentierter Code.** Wenn etwas zu groß für den Moment ist, schreibe es als eigene Aufgabe in `PLAN.md` und implementiere den Rest vollständig.
- Jedes Rust-Modul mit Unit-Tests. Jeder Shader mit einem Referenzbild-Test (Golden Image, Toleranz definiert). E2E-Test pro Modul.
- Fehlerbehandlung überall explizit: `Result`, `thiserror`, keine `unwrap()` außerhalb von Tests.
- Kommentare auf Deutsch, Code-Bezeichner auf Englisch.
- Bei jeder Bibliothek, die du hinzufügst: Lizenz prüfen und in `THIRD_PARTY.md` eintragen. Nichts mit GPL im Kern, außer du weist mich ausdrücklich darauf hin.
- Wenn eine Anforderung technisch nicht sinnvoll umsetzbar ist, sag es mir direkt mit Begründung und Alternativvorschlag, statt eine schwache Version zu bauen.
- Frage nach, wenn eine Entscheidung den Umfang wesentlich verändert. Frage nicht nach Kleinigkeiten, entscheide und dokumentiere sie in `DECISIONS.md`.

---

## 7. Definition of Done pro Feature
1. Funktioniert auf Windows, macOS und Linux.
2. Test vorhanden und grün.
3. Tastenkürzel vergeben und im Cheatsheet.
4. In `FEATURES.md` abgehakt.
5. Rückgängig/Wiederholen funktioniert.
6. In der EDL serialisierbar und nach Neustart identisch reproduzierbar.
7. Performance-Budget aus 2.4 eingehalten.
