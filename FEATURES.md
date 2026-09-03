# FEATURES.md — Feature-Matrix Aperture X

Vollständige Feature-Liste aus `SPEC.md`, ein Punkt pro Zeile mit Checkbox, Ziel-Phase (aus `SPEC.md` Abschnitt 5) und Status. Phasen-Zuordnung ist die sinnvollste Interpretation der Phasenbeschreibung und wird beim Start der jeweiligen Phase in `PLAN.md` verfeinert, falls nötig — Änderungen daran gehören dann in `DECISIONS.md`.

**Status-Werte:** `Nicht begonnen` · `In Arbeit` · `Fertig` · `Fertig (abweichend, siehe DECISIONS.md)`

---

## 3.1 Modul BIBLIOTHEK
<!-- Phasen-Zuordnung korrigiert (siehe DECISIONS.md ADR-0022, analog zu
     ADR-0011 bei Phase 2): SPEC.md §5s Phase-3-Satz nennt nur "Import,
     Ordner, Raster, Filmstreifen, Vorschau-Generierung, Bewertungen/
     Flaggen/Farben, Sammlungen, Filter, Metadaten-Panel, FTS-Suche" —
     §3.1s vollständiger Feature-Katalog (der komplette Lightroom-
     BIBLIOTHEK-Umfang) hatte aber deutlich mehr Punkte auf Phase 3
     getaggt. Die unten nicht mehr auf Phase 3 stehenden Punkte sind auf
     die Phase verschoben, zu der sie inhaltlich am besten passen.
     Zwei Punkte (Duplikaterkennung per Hash, Sortierung) waren bei
     dieser ersten Korrektur übersehen worden und sind erst bei der
     Phase-3-Abnahme (Schritt 7) nachträglich umgetaggt worden, siehe
     DECISIONS.md ADR-0026 — auf ausdrücklichen Nutzerwunsch aber noch
     in Phase 3 nachgezogen worden (zusammen mit drei weiteren im
     Abschlussbericht ehrlich benannten Lücken), siehe ADR-0027. -->

- [x] Import: Ordner scannen, Metadaten lesen, Thumbnails erzeugen (Basisfunktion) — Phase 1 — Status: Fertig
- [x] Import mit Kopieren/Verschieben/Hinzufügen — Phase 3 — Status: Fertig (Backend seit Phase 3; die damalige Fertig-Markierung war vorschnell — es gab bis Phase 5 Schritt 9 **kein** Frontend dafür, siehe `ARCHITECTURE.md` §9 und `DECISIONS.md` ADR-0031 Punkt 7. Jetzt über `ImportDialog.tsx` erreichbar)
- [x] Import mit DNG-Konvertierung — Phase 8, in Phase 11 Schritt 1 nachgerüstet — Status: Fertig (abweichend, siehe ADR-0038) — zum Zeitpunkt von ADR-0034 gab es keine schreibfähige DNG-Crate in reinem Rust (`dng` war reiner Lesezugriff); `gamut-dng` (1.0.0) existiert inzwischen und schreibt ein dekodiertes RAW als valide Linear-DNG. Abweichend vom ursprünglichen Plan als Knopf im Entwickeln-Panel statt als Import-Dialog-Checkbox umgesetzt (`convert_photo_to_dng`, siehe `commands.rs`s Moduldoku) — eine Linear-DNG muss aus den unveränderten RAW-Daten entstehen, nicht aus einem ggf. bereits während des Imports bearbeiteten Ergebnis
- [x] Import-Presets — Phase 3 — Status: Fertig (dieselbe Korrektur wie die Zeile darüber: Backend seit Phase 3, Frontend erst Phase 5 Schritt 9)
- [x] Automatisches Umbenennen mit Token-System — Phase 3 — Status: Fertig (dieselbe Korrektur: der Token-Editor mit Live-Vorschau kam erst mit `ImportDialog.tsx` in Phase 5 Schritt 9 hinzu)
- [x] Duplikaterkennung per exaktem Hash — Phase 3 — Status: Fertig (siehe ADR-0027: `content_hash`-Spalte existierte bereits seit Phase 1, wird jetzt per Streaming-SHA-256 beim Import befüllt; reine Anzeige, blockiert den Import nicht)
- [x] Beobachteter Ordner / Auto-Import — Phase 12 Schritt 7 — Status: Fertig (siehe DECISIONS.md ADR-0039-Nachtrag III) — Hintergrund-Task (`watched_folder_worker`, Polling wie beim bestehenden Export-Queue-Worker, kein natives Datei-System-Watcher-Crate) prüft einen in den Einstellungen konfigurierten Ordner regelmäßig und importiert neue Dateien automatisch im Modus "Hinzufügen" (kein Kopieren/Verschieben); teilt sich die Import-Sperre mit einem manuellen Import, damit sich beide nie überschneiden
- [x] Duplikaterkennung per Perceptual Hash, Duplikat-Assistent mit Auto-Auswahl bester Version — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 1) — hasht die bereits vorhandene 256px-Miniaturansicht (`image_hasher`, in `apx-app`) statt jedes Mal neu zu dekodieren, Fotos ohne generierte Miniaturansicht werden übersprungen; Duplikat-Assistent ist eine reine Heuristik (Auflösung → Dateigröße → Bewertung), keine Inhaltsanalyse
- [x] Ordnerbaum (Basis-Anzeige, Fotoanzahl je Ordner) — Phase 1 — Status: Fertig (flache Liste, kein Baum — echte Hierarchie/Synchronisation ist Phase 3, siehe Zeile darunter)
- [x] Ordnerbaum-Synchronisation (echte Hierarchie über `parent_id`) — Phase 3 — Status: Fertig (siehe DECISIONS.md ADR-0027: Import legt jetzt die volle Verzeichniskette bis zum gewählten Import-Ordner bzw. bei Copy/Move bis zum Zielordner an, statt nur den unmittelbaren Elternordner)
- [x] Ordner fehlend/wiederfinden — Phase 3 — Status: Fertig
- [x] Sammlungen (manuell, feste Reihenfolge) — Phase 3 — Status: Fertig
- [x] Sammlungssätze, intelligente Sammlungen mit verschachtelten UND/ODER-Regeln, Zielsammlung — Phase 9, echter Regelbaum seit Phase 13 Schritt 7 — Status: Fertig (abweichend, siehe PLAN.md Schritt 1) — `collection_folders` spiegelt `preset_folders`; intelligente Sammlungen nutzen seit Phase 13 Schritt 7 einen echten, beliebig verschachtelbaren UND/ODER-Regelbaum (`apx_catalog::FilterNode`, siehe `DECISIONS.md` ADR-0040-Nachtrag V — davor eine flache UND-Verknüpfung der `FilterCriteria`-Felder), Mitgliedschaft weiterhin live berechnet, jetzt in-memory über `FilterNode::matches` statt reiner SQL-WHERE-Klausel; „Zielsammlung" (automatisches Hinzufügen neu importierter Fotos) weiterhin nicht umgesetzt
- [x] Stapel (automatisch nach Zeit, manuell) — Phase 9 — Status: Fertig — manuell aus der aktuellen Auswahl oder automatisch per Zeitfenster über `captured_at`; kein inline Ein-/Ausklappen im Raster, Verwaltung über den `LibraryOrganizeDialog`
- [x] Virtuelle Kopien — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 1) — `photos.source_photo_id` (nullable Selbstreferenz) statt einer separaten Tabelle, nimmt an edit_history/keywords/collections/snapshots/rating unverändert teil
- [x] Bewertung 0–5 — Phase 3 — Status: Fertig
- [x] Farbmarkierungen (fester Grundsatz) — Phase 3 — Status: Fertig (feste Palette aus 5 Farben: rot/gelb/grün/blau/violett)
- [x] Farbmarkierungen erweiterbar auf beliebig viele, benannt — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 1) — neue `color_label_definitions`-Tabelle ersetzt die frühere feste `ALLOWED_COLOR_LABELS`-Konstante, verwaltet über den `LibraryOrganizeDialog`; die bestehende kompakte `ColorLabelPicker`-Auswahl in Raster-Zellen/Metadaten-Panel zeigt weiterhin nur die fünf Standardfarben, neue Farben sind über den Dialog setzbar, aber noch nicht dort wählbar
- [x] Flaggen — Phase 3 — Status: Fertig
- [x] Schlagworte (flache Liste, ohne Hierarchie) — Phase 3 — Status: Fertig
- [x] Schlagworthierarchie (Eltern/Kind, Synonyme), Tag-Regeln (bedingte Auto-Tags) — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 2) — `keywords.parent_id`/`synonyms`, neue `tag_rules`-Tabelle (`conditions_json` im `PresetCondition[]`-Vertrag der Import-Presets); Regeln sind über `MetadataDialog.tsx` anlegbar/verwaltbar, aber noch nicht automatisch am Import-Ablauf verdrahtet (reiner Verdrahtungsschritt für später); Export-Steuerung/Auto-Vervollständigung/separate Schlagwortvorschläge nicht Teil dieses Schritts
- [x] Auto-Tagging — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 6) — regelbasierte Vorschläge aus Segmentierungs-Heuristiken (Himmel-/Personen-Flächenanteil) + EXIF-Faustregeln (ISO/Blende/Brennweite) statt echter Bildklassifikation; `apx-ai::tagging::suggest_tags` schreibt nichts selbst in den Katalog, das Frontend übernimmt Vorschläge über das bestehende `add_photo_keyword`
- [x] Metadaten-Panel (Basisfelder lesen, Bewertung/Flagge/Farbe/Schlagworte editieren) — Phase 3 — Status: Fertig
- [x] Undo/Redo für Bibliotheks-Metadaten (Bewertung/Flagge/Farbe/Schlagworte/Sammlungsmitgliedschaft) — Phase 3 — Status: Fertig (siehe DECISIONS.md ADR-0027; deckt bewusst nicht Sammlung anlegen/umbenennen/löschen ab)
- [x] Stapel-Metadatenbearbeitung, IPTC-artige Felder (Titel/Bildunterschrift/Copyright/Urheber), Sidecar-Export (.xmp) — Phase 9, voller EXIF/IPTC-Editor Phase 12 Schritt 4 — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0039) — `photos.title`/`caption`/`copyright`/`creator` weiterhin als feste Spalten editierbar über `MetadataDialog.tsx`; zusätzlich `photos.custom_metadata_json` (Migration 10) als generisches Key-Value-Feld für die üblichen IPTC-Kernfelder (`apx_catalog::iptc::WELL_KNOWN_FIELDS`: Überschrift/Anweisungen/Quelle/Auftragskennung/Stadt/Bundesland/Land/Ort/Ereignis/Genre) plus beliebige frei benannte Zusatzfelder, ohne dass jedes neue Feld eine weitere Migration braucht. Mehrfachauswahl aktualisiert weiterhin alle markierten Fotos. `apx_export::xmp::write_sidecar` schreibt diese Zusatzfelder mit in die `.xmp`-Sidecar: wohlbekannte Schlüssel auf die echte `photoshop:`/`Iptc4xmpCore:`/`Iptc4xmpExt:`-XMP-Eigenschaft abgebildet (als einfaches Attribut, nicht die volle Lang-Alt-/Bag-Struktur), frei benannte Schlüssel im eigenen `apx:`-Namensraum statt eine nicht existierende Adobe-Eigenschaft zu erfinden. Metadaten-Presets über die generische `templates`-Tabelle bleiben zurückgestellt
- [x] Volltextsuche (FTS5) über Dateiname, Kamera, Objektiv — Phase 3 — Status: Fertig
- [x] Rasteransicht — Phase 3 — Status: Fertig
- [x] Lupe/Einzelbildansicht (Basis-Viewer) — Phase 1 — Status: Fertig
- [x] Vergleichsansicht (bis 9 Fotos, Bewertung/Flagge direkt bedienbar) — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 3) — `CompareGridView.tsx`, zeigt die Standard-Vorschau statt eines Live-Renders und ohne synchronisierten Zoom/Pan (bewusste Vereinfachung, siehe dessen Moduldoku)
- [x] Übersichtsansicht (separater Modus) — Phase 9, in Phase 11 Schritt 3 nachgerüstet — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0038) — dritter `centerView`-Modus `"overview"` (`GridView.tsx`s `variant`-Prop): größere Kacheln (360px), reduziertes Metadaten-Overlay (nur Dateiname statt Bewertungs-/Flaggen-/Farb-Widgets), Klick wählt nur `selectedPhotoId` statt Mehrfachauswahl — teilt sich Virtualisierung/Fotoliste mit der bestehenden Rasteransicht statt einer eigenen Implementierung
- [x] Personenansicht (Gesichtserkennung) — Phase 9, in Phase 11 Schritt 5 nachgerüstet, echte Gesichts-Embeddings seit Phase 13 Schritt 8 — Status: Fertig — `apx-ai::people::PersonEmbedder` (hinter dem standardmäßig ausgeschalteten Cargo-Feature `people`, siehe `DECISIONS.md` ADR-0040-Nachtrag VI): echte Gesichtserkennung (`dlib::get_frontal_face_detector`) + 5-Punkt-Ausrichtung + 128-dimensionales Embedding (`dlib`s eigenes, vom Autor gemeinfrei erklärtes Modell — InsightFace/OpenCV-`SFace` real geprüft und wegen ungeklärter/nicht-kommerzieller Lizenzlage verworfen), Auto-Zuordnung neu erkannter Gesichter zur nächstliegenden benannten Person per Schwellenwert-Clustering auf dem euklidischen Embedding-Abstand (`migrations/0011_people.sql`: `people`/`face_detections`-Tabellen). Opt-in-Modell-Download, kein Bundling im Installer. Die frühere Hautton-Heuristik (`apx_ai::faces::detect_face_regions`, `list_people_groups`-Command, neutrale "Gruppe"-Bezeichnung) bleibt additiv als Fallback bestehen, wenn das `people`-Feature nicht kompiliert oder keine Modelle hinterlegt sind — **ehrlich begrenzt**: standardmäßig ist das Feature aus (Systembibliothek `libdlib` fehlt nicht überall), der ausgelieferte Installer bindet es bislang nicht ein (dieselbe Einschränkung wie `apx-tether`s `tethering`-Feature)
- [x] Filterleiste (Text, Attribut, Metadaten, kombiniert) — Phase 3 — Status: Fertig (siehe DECISIONS.md ADR-0027: Text- und Attributfilter [inkl. Kameramodell] sind jetzt per UND kombinierbar, nicht mehr alternativ wie in ADR-0026)
- [x] Filter-Presets — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 3) — nutzt die generische `templates`-Tabelle (`kind = "filter"`) aus Phase 8 Schritt 8, keine neue Tabelle nötig
- [x] Sortierung nach beliebigem Feld — Phase 3 — Status: Fertig (siehe ADR-0027: client-seitig, Dateiname/Aufnahmedatum/Bewertung/Dateigröße/Kameramodell, fehlende Werte immer ans Ende)
- [x] Schnellentwicklung im Raster — Phase 9, in Phase 11 Schritt 3 nachgerüstet — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0038) — `QuickDevelopOverlay.tsx`: die sieben Phase-2-Basisregler als kompaktes Overlay auf einer Kachel der Übersichtsansicht bei Hover/Auswahl, committet über denselben `apply_develop_edit`-Pfad wie das Entwickeln-Panel, aber über einen eigenen Store-Zustand (`quickDevelopEdl`), damit ein offenes Entwickeln-Panel für ein anderes Foto nicht überschrieben wird
- [x] Vorschau-Cache-Verwaltung (Größe einsehen, leeren) — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 3) — `preview_cache_stats`/`clear_preview_cache` über `AppPaths::preview_cache_dir()`; „1:1"-Cache-Größe separat von der Standard-Vorschau nicht unterschieden (beide liegen im selben Verzeichnis)
- [x] Smart Previews, Offline-Bearbeitung über Smart Previews — Phase 9, in Phase 11 Schritt 4 nachgerüstet — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0038) — `generate_smart_previews`-Command schreibt je Foto ein festes, verkleinertes JPEG (2560px lange Kante) nach `AppPaths::smart_preview_dir()`; `apx-app::protocol::resolve_source_path` (der eine Ort, den jeder Rendering-Pfad durchläuft) fällt darauf zurück, wenn die Originaldatei nicht erreichbar ist — Vorschau/Vollbild/Entwickeln-Route bekommen den Fallback transparent, da `apx_raw::decode`/`decode_linear` ein Smart-Preview-JPEG wie jede andere Fallback-Bilddatei behandelt. Viewer zeigt „Offline (Smart Preview)", abgeleitet rein aus dem bestehenden `photo.missing`-Feld plus einem tatsächlich gerenderten Bild — kein neues Backend-Signal nötig
- [x] Sekundäres Display mit unabhängiger Ansicht — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 3) — eigenes `WebviewWindow` mit `?secondaryPhoto=<id>`, zeigt ein statisches Bild über den bestehenden `image/...`-Protokoll-Handler (kein Live-Sync mit dem Hauptfenster); die Tauri-Fenster-Erstellung selbst ist in dieser Sandbox ohne echten Tauri-Runtime-Host nicht ausführbar getestet (gleiche Einschränkung wie bei anderen Tauri-Fenster-/Dialog-Aufrufen dieses Projekts)
- [x] Katalog-Statistiken-Dashboard — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 3) — `repository::stats`: Gesamtzahl/-größe, Aufnahmezeitraum, Bewertungsverteilung, Top-Kameramodelle/-Objektive (je höchstens 8); schließt virtuelle Kopien konsequent aus

## 3.2 Modul ENTWICKELN — Globale Werkzeuge

### Grundeinstellungen
<!-- Phasen-Zuordnung korrigiert (siehe DECISIONS.md ADR-0011): SPEC.md §5s
     Phase-2-Satz nennt nur die folgenden sieben Regler namentlich; Textur/
     Klarheit/Dunst entfernen/Dynamik/Sättigung gehören daher zu Phase 4,
     nicht zu Phase 2. Aus demselben Grund gehören Pipette und
     Kamera-Presets (Teil von §3.2s vollem Weißabgleich-Feature, aber
     nicht Teil des Phase-2-Satzes) ebenfalls erst zu einer späteren
     Phase — Weißabgleich selbst (Temperatur+Tint als Regler) ist für
     Phase 2 fertig. -->
- [x] Weißabgleich (Temperatur, Tint) — Phase 2 — Status: Fertig
- [x] Weißabgleich-Pipette (Klick ins Bild) — Phase 4 — Status: Fertig (bewusst vereinfacht: rechnet auf dem gamma-kodierten Anzeigebild statt linearem Kamera-RGB, siehe `frontend/src/lib/whiteBalancePicker.ts`)
- [x] Weißabgleich-Kamera-Presets (Tageslicht/Bewölkt/…) — Phase 4 — Status: Fertig (feste, nicht kamera-kalibrierte Presets, siehe ADR-0028)
- [x] Belichtung — Phase 2 — Status: Fertig
- [x] Kontrast — Phase 2 — Status: Fertig
- [x] Lichter — Phase 2 — Status: Fertig
- [x] Tiefen — Phase 2 — Status: Fertig
- [x] Weiß — Phase 2 — Status: Fertig
- [x] Schwarz — Phase 2 — Status: Fertig
- [x] Textur — Phase 4 — Status: Fertig
- [x] Klarheit — Phase 4 — Status: Fertig
- [x] Dunst entfernen — Phase 4 — Status: Fertig (vereinfachtes Modell, kein Dark-Channel-Prior, siehe `crates/apx-pipeline/src/edl/v2.rs`)
- [x] Dynamik — Phase 4 — Status: Fertig
- [x] Sättigung — Phase 4 — Status: Fertig

### Gradationskurve
- [x] Punktkurve — Phase 4 — Status: Fertig (monotone kubische Fritsch-Carlson-Spline, siehe `crates/apx-pipeline/src/stages/curves.rs`)
- [x] Parametrische Kurve — Phase 4 — Status: Fertig (vereinfachtes Gauß-gewichtetes Vier-Zonen-Modell statt echter verschiebbarer Split-Punkte, siehe `curves.rs`)
- [x] RGB-Verbundkurve + einzelne Kanäle — Phase 4 — Status: Fertig
- [x] Luminanz-Kurve — Phase 4 — Status: Fertig
- [x] Numerische Punkteingabe — Phase 4 — Status: Fertig
- [x] Kurven-Presets — Phase 4 — Status: Fertig (4 feste Presets: Linear/Leichter Kontrast/Starker Kontrast/Negativ)

### HSL / Farbe
- [x] HSL für 8 Standardbereiche — Phase 4 — Status: Fertig (Gauß-gewichtete Farbton-Bandzuordnung statt scharfer Grenzen, siehe `crates/apx-pipeline/src/stages/hsl_color_mixer.rs`)

### Farbmischer erweitert
- [x] Frei definierbare Farbbereiche per Klick im Bild — Phase 4 — Status: Fertig (auf 8 Regionen begrenzt, siehe `hsl_color_mixer.rs`s Moduldoku)

### Farbklassifizierung / Color Grading
- [x] Farbräder Schatten/Mitteltöne/Lichter/Global — Phase 4 — Status: Fertig (Gauß-gewichtete Tonwertzonen statt fester Umschlagpunkte, siehe `crates/apx-pipeline/src/stages/color_grading.rs`)
- [x] Luminanz und Mischung pro Rad — Phase 4 — Status: Fertig
- [x] Balance, Überblendung — Phase 4 — Status: Fertig (Balance verschiebt das Gewicht zwischen Schatten-/Lichter-Zonen statt deren Zentren zu verschieben, siehe Moduldoku in `color_grading.rs`)

### Details
- [x] Schärfung (Betrag, Radius, Detail, Maskierung) — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028: Unsharp-Masking mit ganzzahligem Box-Filter-Radius statt eines echten Gauß-Kerns, `sharpen_masking` über eine `smoothstep`-Schwelle statt echter Kantenerkennung, siehe `stages/details.rs`)
- [x] Luminanzrauschen (Betrag, Detail, Kontrast) — Phase 4 — Status: Fertig (abweichend: einfacher fester 3×3-Box-Weichzeichner statt eines echten bilateralen Filters)
- [x] Farbrauschen (Betrag, Detail, Glättung) — Phase 4 — Status: Fertig (abweichend: teilt sich den Luminanz-Kantenwert mit der Luminanzrauschen-Reduktion statt eines eigenen Chroma-Kantenmaßes)
- [x] Deconvolution-Schärfung — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028: Potenzfunktions-Verstärkung des Hochpass-Anteils als bewusst einfacher Stand-in, kein echtes iteratives Entfaltungsverfahren)

### Objektivkorrekturen
<!-- Scope-Präzisierung siehe DECISIONS.md ADR-0028: eine echte
     Adobe-LCP-kompatible Profildatenbank ist ohne die nötigen Testdaten
     ein eigenständiges Mammutprojekt. Phase 4 liefert ein eigenes,
     minimales Profilformat (handgepflegtes JSON, wenige Beispielprofile,
     Zuordnung per EXIF-Objektiv-/Kamerastring) plus die vollen manuellen
     Regler; echter Adobe-Profil-Import wird auf eine spätere Phase
     verschoben. -->
- [x] Profilbasierte Korrektur (Datenbank + Import) — Phase 4, echte LensFun-Datenbank + Auto-Anwendung Phase 12 Schritt 3 Teil A — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0039) — `apx_pipeline::lens_profiles` nutzt jetzt die echte, offene LensFun-Datenbank (`lensfun`-Crate, Tausende real kalibrierte Objektive) statt der drei ursprünglichen Beispielprofile (die bleiben unter ihrer alten `id` auflösbar, für Altbestand); Verzeichnung/Vignette/CA werden ehrlich aus LensFuns realer `Modifier`-Pixelmathematik an der Bildecke zurückgerechnet (kein 1:1-Koeffizienten-Import, siehe `lens_profiles.rs`s `derive_lens_correction_values`-Moduldoku), nicht mehr per Adobe-LCP-Import (weiterhin ungelöstes proprietäres Format). Wird jetzt automatisch beim ersten Öffnen eines Fotos aus dem EXIF-Objektivstring zugeordnet (nur bei noch nie bearbeiteten Fotos, überschreibt nie eine bewusste Nutzerwahl), zusätzlich ein manueller „Automatisch erkennen"-Knopf im Entwickeln-Panel
- [x] Objektiv-Kalibrier-Assistent für Objektive außerhalb der LensFun-Datenbank — Phase 12 Schritt 3 Teil B — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0039) — Dialog „Objektiv kalibrieren": Nutzer markiert mehrere Punkte entlang in der Realität gerader Linien direkt auf einer Bildvorschau, `apx_ai::lens_calibration` sucht per Rasterverfeinerung den einen Verzeichnungskoeffizienten, der diese Linien nach der Entzerrung am geradesten macht (klassische Optimierung, kein gelerntes Modell — ehrlich als „aus eigenen Kalibrierfotos berechnet" beschriftet, nicht „KI-generiert"). **Bewusst kein volles Zhang-Verfahren** (keine automatische Schachbrett-Eckenerkennung, keine Homografie-Schätzung, kein Mehrparameter-Kameramodell — siehe `lens_calibration.rs`s Moduldoku) und **nur Verzeichnung**, keine Vignette/CA. Ergebnis lebt direkt im EDL (`custom_distortion_k1`), keine separate Profildatenbank — Wiederverwendung auf andere Fotos über die vorhandene Einstellungen-kopieren-Funktion
- [x] Chromatische Aberration (auto + manuell) — Phase 4 — Status: Fertig (radiale Kanalverschiebung um den Bildmittelpunkt, `auto_ca` nutzt Profilwerte statt echter Kantenerkennung)
- [x] Vignettierung — Phase 4 — Status: Fertig (radiale Aufhellung zum Bildrand, additiv mit dem Profilwert kombiniert)
- [x] Verzeichnung — Phase 4 — Status: Fertig (abweichend, siehe ADR-0030: Ein-Koeffizienten-Radialmodell statt eines mehrparametrigen Brown-Conrady-Modells)
- [x] Perspektive/Upright (Auto, Level, Vertical, Full, Guided) — Phase 4 — Status: Fertig (abweichend, siehe ADR-0030: „Auto"/„Level"/„Vertical"/„Full" sind wählbare, aber wirkungslose Platzhalter — echte Kantenerkennung ist eine CV-Aufgabe außerhalb des Stacks; „Guided" mittelt die ersten zwei Hilfslinien zu einer einfachen Dreh-Korrektur statt einer echten Mehrlinien-Homografie, Hilfslinien per Zahlenfeld statt Viewer-Klick-Interaktion eingegeben)
- [x] Manuelle Transformation (V/H, Drehen, Seitenverhältnis, Skalieren, Versatz) — Phase 4 — Status: Fertig (V/H als Scherung statt echter Homografie, Ausgabegröße bleibt unverändert — Randpixel geklemmt, echtes Zuschneiden folgt in Schritt 11, siehe ADR-0030)

### Effekte
- [x] Nachträgliche Vignettierung — Phase 4 — Status: Fertig (abweichend: `roundness` blendet nur in Richtung „runder", `feather`/`midpoint` steuern eine einzelne `smoothstep`-Übergangszone statt eines mehrstufigen Verlaufs, siehe `stages/effects.rs`)
- [x] Körnung — Phase 4 — Status: Fertig (abweichend: deterministischer Ganzzahl-Hash aus der Pixelposition statt echten mehrstufigen Frequenz-Rauschens — dadurch automatisch stabil über Re-Renders, kein Flackern)

### Kalibrierung
- [x] Prozessversion — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028: nur `V1` existiert, reiner Vorwärtskompatibilitäts-Platzhalter ohne aktuellen Effekt, siehe `edl/v2.rs`s Moduldoku)
- [x] Schattentönung — Phase 4 — Status: Fertig (additive Grün-/Magenta-Verschiebung, gewichtet mit einer festen Gauß-Schatten-Zone statt eines editierbaren Umschlagpunkts, siehe `stages/calibration.rs`)
- [x] Primärfarben-Regler R/G/B (Farbton, Sättigung) — Phase 4 — Status: Fertig (Gauß-gewichtete Farbton-Bänder um 0°/120°/240° statt echter Matrixrotation, siehe Moduldoku)
- [x] Kamera-Profile (kleine eingebaute Liste) inkl. DCP-Import — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028: `CAMERA_PROFILES` ist eine kleine handgepflegte Liste mit festem Sättigungs-/Kontrast-Bias je Profil, kein echter DCP-/ICC-Profilwechsel; DCP-Import selbst auf spätere Phase verschoben — dasselbe Adobe-Format-Problem wie bei Objektivprofilen)

### Geometrie
- [x] Freistellen (Presets, eigene Verhältnisse, Rasterüberlagerungen) — Phase 4 — Status: Fertig (abweichend, siehe ADR-0030: „Spirale" als verschachtelte Goldener-Schnitt-Rechtecke statt echter logarithmischer Spirale, „Diagonalen" mit zwei statt vier Linien; `stages/geometry.rs` extrahiert das Zuschnitt-Rechteck pixel-genau ohne Resampling)
- [x] Winkel-Werkzeug — Phase 4 — Status: Fertig (bilinear abgetastete Drehung um den Bildmittelpunkt, Randpixel geklemmt statt schwarz gefüllt)
- [x] Auto-Ausrichtung am Horizont — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028/ADR-0030: dokumentierter No-op-Platzhalter in dieser Stufe — die EXIF-Ausrichtung läuft bereits vor der EDL-Pipeline in `apx-raw`, ein weiteres automatisches Ausrichten bräuchte echte Kantenerkennung außerhalb des Stacks)

### Reparatur
<!-- Scope-Präzisierung siehe DECISIONS.md ADR-0028: Auto-Quellenfindung
     und inhaltsbasiertes Füllen für größere Bereiche sind fortgeschrittene
     Computer-Vision-Algorithmen (vergleichbar mit PatchMatch/Content-Aware
     Fill) und werden auf eine spätere Phase verschoben. Phase 4 liefert
     manuelles Klonen/Reparieren (Pinsel mit Quellpunkt, Radius, Deckkraft,
     weicher Kante). -->
- [x] Bereichsreparatur klonen/reparieren — Phase 4 — Status: Fertig (abweichend, siehe ADR-0028: Reparieren per vereinfachtem Tiefpass/Hochpass-Überblenden statt echten Poisson-Blendings, Pfad-Abstand als minimaler Stützpunkt-Abstand statt echter Punkt-zu-Liniensegment-Distanz, Striche sequenziell statt als ein Fused-Pass angewendet, siehe `stages/repair.rs`)
- [x] Auto-Quellenfindung — Phase 7 — Status: Fertig — `apx-ai::repair_analysis::suggest_source_point`, normierte Kreuzkorrelation über einen festen Kandidatenring; im Frontend per Checkbox „Quelle automatisch vorschlagen" aktivierbar, ersetzt dann den manuellen Quellpunkt-Klick
- [x] Sensorflecken-Visualisierung — Phase 7 — Status: Fertig — `apx-ai::repair_analysis::detect_spots`, Blob-Erkennung per lokaler Kontrast-Anomalie gegen ein weichgezeichnetes Referenzbild; reine Analyse (kein automatisches Committen), „Reparieren" übernimmt einen Fund als `ContentAwareFill`-Strich
- [x] Inhaltsbasiertes Füllen — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 4) — vereinfachtes PatchMatch (Nächster-Nachbar-Vorbelegung, Zufallsinit, Propagation, Zufallssuche) statt eines vollständigen Multi-Skalen-Verfahrens; bleibt bewusst in `apx_pipeline::stages::repair` als vierter Reparatur-Modus (render-zeitlich, nicht `apx-ai`), da es bei jedem Rendering läuft statt nur einmal auf Knopfdruck

### Histogramm, Zielwerkzeuge & KI-Verbesserung
<!-- Neu ergänzt auf ausdrücklichen Nutzerwunsch (Screenshot von Lightroom
     Classics Histogramm-/Basic-Panel als Vorlage): das Live-Histogramm
     selbst plus zehn weitere Lightroom-Fähigkeiten, die in `SPEC.md`/
     `FEATURES.md` bisher nirgends vorkamen — keine reine Lücken-Korrektur
     wie bei den ADR-0011/-0022/-0026-Nachträgen oben, sondern eine echte
     Scope-Erweiterung. Alle elf Punkte sind noch ungeplant im Detail
     (keine ADR, kein Unterbau) — Phase 9 als vorläufige Einordnung, siehe
     `PLAN.md`s neuer Abschnitt „Backlog-Ergänzung für Phase 9 (auf
     Nutzerwunsch, außerhalb der Reihe)". -->
- [x] Live-Histogramm (RGB + Luminanz, aktualisiert sich sofort bei jedem Reglerzug) — Phase 9 — Status: Fertig (siehe PLAN.md Schritt 4) — `lib/histogram.ts::computeHistogram` zählt die vier Kanäle aus dem bereits vorhandenen `render_rgba8`-Ausgabepuffer (`developFrame`), reiner Frontend-Rechenschritt, kein neuer Rendering-Pfad; Luminanz über Rec.-709-Gewichte, wie `lib/softProof.ts`s Gamut-Warnung
- [x] Clipping-Warnungen (Lichter-/Tiefen-Dreiecke anklickbar, Rot-/Blau-Überlagerung im Bild) — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 4) — die beiden Dreieck-Knöpfe schalten dieselbe Rot-/Blau-Überlagerung um (kein getrenntes Nur-Tiefen-/Nur-Lichter-Umschalten); die Überlagerung selbst ist ein zweites, transparentes 2D-Canvas über dem WebGL-Viewer statt eines Eingriffs in `QuadRenderer`s Zeichenpfad
- [x] Punktfarbmesser (RGB-Wert unter dem Mauszeiger live anzeigen, wie ein Densitometer) — Phase 9 — Status: Fertig (siehe PLAN.md Schritt 4) — läuft passiv beim Überfahren des Bilds mit, kein eigener Werkzeug-Modus nötig (anders als die klickbasierten Pipetten für Weißabgleich/Farbmischer)
- [x] Zielgerichtetes Anpassungswerkzeug (TAT) für Kurven und HSL — Phase 9, in Phase 11 Schritt 6 nachgerüstet — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0038) — neue TAT-Werkzeugleiste im Viewer (nur bei offenem Entwickeln-Panel): „TAT: Kurve" (Dropdown wählt den Kanal) verschiebt beim Klick+Zug den nächstgelegenen Kurvenpunkt oder legt bei Bedarf einen neuen an; „TAT: HSL" verschiebt die Luminanz des unter dem Cursor gesampelten Farbtons (`nearestHslBand`, dieselben acht Bandzentren wie `hsl_color_mixer.rs`) — vertikaler Zug skaliert mit der sichtbaren Bildhöhe (Lightroom-Konvention: nach oben = erhöhen)
- [x] Schwarzweiß-Umwandlung mit eigenem 8-Kanal-Mixer (Treatment-Umschalter Farbe/Schwarzweiß, ein Luminanz-Regler je Farbband) — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 5) — additiv zu Schema-Version 3 (`#[serde(default)]`, kein v4-Sprung), läuft nach der Farbraum-Konvertierung auf dem fertigen RGBA8-Puffer wie `curves`; Bandgewichtung per Gauß-Kurve über dieselben acht Farbton-Bänder wie der HSL-Mixer; Standardwerte sind eine grobe eigene Näherung, keine Rekonstruktion von Adobes proprietären Zahlen
- [x] Auto-Ton per Ein-Klick (Histogramm-Perzentil-Heuristik statt Regler von Hand setzen, kein LLM) — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 5) — setzt nur Belichtung (Median-Perzentil → 18 % Grau) und Kontrast (Perzentil-Spanne → Kontrast-Stärke); Lichter/Tiefen/Weiß/Schwarz bleiben unverändert (eine algebraische Umkehrung ihrer tonwertkurvenartigen Wirkung wäre nur geraten); kein separates Auto-Weißabgleich in diesem Schritt
- [x] Navigator-Miniaturansicht (kleine Übersichtskarte zeigt die Zoom-Ausschnittsposition bei starker Vergrößerung) — Phase 9 — Status: Fertig (siehe PLAN.md Schritt 4) — zeigt die bestehende Thumbnail-Vorschau mit einem Rahmen für den sichtbaren Ausschnitt (Umkehrung von `lib/viewerMath.ts::imageOrigin`), kein eigenes zweites Bild-Rendering
- [x] Entrauschung über die volle Bildfläche („Denoise"-artig) — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 6, DECISIONS.md ADR-0035 Punkt 6) — dasselbe ONNX-Beschaffungsproblem wie in ADR-0033 bleibt ungelöst, deshalb **kein** neuronales Modell: echter kantenerhaltender Bilateral-Filter (`apx_ai::denoise::bilateral_filter_rgba8`) auf dem voll aufgelösten Rendering, Ergebnis als neue PNG-Datei neben dem Original — UI-Beschriftung bewusst ohne „KI"/„AI"
- [x] Hochskalierung / Detailverbesserung („Super Resolution"-artig) — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 6, DECISIONS.md ADR-0035 Punkt 6) — dasselbe ONNX-Beschaffungsproblem wie oben, deshalb **kein** gelerntes Modell: kantengerichtete 2×-Interpolation (`apx_ai::upscale::edge_directed_upscale_2x_rgba8`, wählt je Zwischenpixel die glattere Diagonalrichtung) statt stumpfem bikubischem Resampling — UI-Beschriftung bewusst ohne „KI"/„AI"
- [x] Info-Overlay im Vollbild-Modus (Dateiname/EXIF/Bewertung direkt über dem Bild eingeblendet, umschaltbar) — Phase 9 — Status: Fertig (siehe PLAN.md Schritt 5) — bereits vorhandene Anzeige jetzt mit `infoOverlayVisible`-Umschalter (Taste „I", wie Lightroom Classic)
- [x] Bearbeitungs-Pins auf dem Bild (anklickbarer Kreis-Marker direkt an der Stelle jeder lokalen Maske/jedes Verlaufs, fokussiert beim Klick die zugehörige Maske im Panel) — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 5) — nur für Masken mit eindeutiger räumlicher Position (Verlauf/Radial/Pinsel); Farb-/Luminanzbereich-Masken (global, keine Position) und KI-generierte Rasterflächen (Schwerpunkt-Suche im Alpha-Kanal nicht gerechtfertigt) haben bewusst keinen Pin

## 3.3 Modul ENTWICKELN — Lokale Anpassungen
<!-- Scope-Präzisierung siehe DECISIONS.md ADR-0032: SPEC.md §5s
     Phase-6-Satz nennt nur "Pinsel, Verläufe, Bereichsmasken,
     Maskenkombination, Ebenen-Mischmodi" — Tiefenbereich (kein
     Tiefendaten-Zulieferer existiert) und die fünf KI-Masken
     (Phase 7, dieselbe apx-ai/ONNX-Runtime-Integration wie der
     Preset-Generator) fallen aus dem Phase-6-Kern heraus. -->

- [x] Maskentyp Pinsel — Phase 6, Auto-Mask Phase 12 Schritt 2 — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0039) — Auto-Mask dämpft die Deckkraft eines Strichs an starken lokalen Bildkanten (wiederverwendet `masks.rs`s Laplace-Varianz-Kantenmaß aus `BlurDepthApprox`, keine eigene neue Heuristik nötig), als Umschalter im MasksPanel für den *nächsten* gemalten Strich
- [x] Maskentyp Linearer Verlauf — Phase 6 — Status: Fertig
- [x] Maskentyp Radialer Verlauf — Phase 6, Ellipse+Rotation Phase 12 Schritt 2 — Status: Fertig — unabhängige `radius_x`/`radius_y`-Achsen und `angle_degrees`-Rotation waren im Datenmodell/in der Pipeline (`masks.rs`s `radial_gradient_alpha`) schon vorhanden, jetzt auch per Ziehgriff im Viewer erreichbar (drei eigene Griffe statt eines gemeinsamen Radius, siehe DECISIONS.md ADR-0039)
- [x] Maskentyp Farbbereich — Phase 6 — Status: Fertig
- [x] Maskentyp Luminanzbereich — Phase 6 — Status: Fertig
- [x] Maskentyp Tiefenbereich — in Phase 11 Schritt 7 als Alternative nachgerüstet — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0038) — kein echter Tiefendaten-Zulieferer existiert weiterhin (ADR-0032 bleibt in der Sache richtig), stattdessen neuer Maskentyp `BlurDepthApprox`: Laplace-Varianz-Schärfeheuristik in einem gleitenden 5×5-Fenster (`apx-pipeline::stages::masks::relative_sharpness_map`), trennt grob Vordergrund/Hintergrund bei echtem Schärfentiefe-Effekt (offene Blende) — versagt bei durchgehend scharfen Aufnahmen. UI-Beschriftung bewusst „Unschärfe-basierte Tiefennäherung", nicht „Tiefenbereich"
- [x] KI-Motiv-Maske — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 1/2) — Center-Surround-Saliency-Heuristik statt echter ONNX-Modellinferenz (kein legitimer Weg, echte Segmentierungs-Modellgewichte in dieser Umgebung zu beschaffen und mitzuliefern)
- [x] KI-Himmel-Maske — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 1/2) — Farbton-/Helligkeits-/Positions-Heuristik
- [x] KI-Hintergrund-Maske — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 1/2) — Komplement der Motiv-Maske
- [x] KI-Objekte-Maske (Klick-Segmentierung) — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 1/2) — farbtoleranzbasiertes Region-Growing ab einem Klickpunkt, kein gelerntes Instanzsegmentierungs-Modell
- [x] KI-Personen-Maske (Haut, Augen, Brauen, Lippen, Zähne, Haare, Kleidung) — Phase 7 — Status: Fertig, mit Teil-Einschränkung (siehe ADR-0033 Punkt 1/2) — Hautton-Erkennung im YCbCr-Raum als eine einzelne Region; Einzelteile (Augen/Brauen/Lippen/Zähne/Haare/Kleidung getrennt wählbar) bewusst nicht umgesetzt, siehe `PLAN.md`
- [x] Masken kombinieren (Hinzufügen/Subtrahieren/Schneiden) — Phase 6 — Status: Fertig — mehrere `MaskComponent`s je Maske, je mit eigenem `MaskCombine` + Invertieren (siehe `PLAN.md` Schritt 6)
- [x] Pro Maske: alle globalen Regler + Deckkraft/Weichzeichnung/Umkehren/Verfeinern — Phase 6 — Status: Fertig (abweichend, siehe ADR-0032 Punkt 2) — eingegrenzt auf die ton-/farb-/detailbezogenen Werkzeuge (Grundeinstellungen, Kurven, HSL, Farbmischer, Color Grading, Details); Objektivkorrekturen/Effekte/Kalibrierung/Geometrie/Reparatur bleiben bewusst global
- [x] Maskengruppen, Umbenennen, Ein-/Ausblenden, Überlagerungsfarbe — Phase 6, Überlagerung Phase 12 Schritt 1 — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0039) — Gruppen (anlegen/umbenennen/Sichtbarkeit/entfernen) vollständig; `overlay_color` steuert jetzt eine per Taste „O" umschaltbare Masken-Farbüberlagerung im Viewer (`MaskColorOverlay.tsx`, `MasksPanel.tsx`-Knopf) für alle sichtbaren Masken. Bewusste Vereinfachung: rein clientseitige SVG-Näherung der Geometrie (Verläufe als Gradient-Flächen, Pinsel als eingefärbte Striche), keine echte pixelgenaue Pipeline-Alpha (`compose_mask_alpha`) — `combine`/`invert`/`feather` einzelner Komponenten fließen deshalb nicht ein, nur die Gesamtdeckkraft. Farbbereich-/Luminanzbereich-/KI-erzeugte/Tiefennäherungs-Masken haben (noch) keine Vektorform und bleiben ohne Überlagerung
- [x] Maske duplizieren / auf anderes Foto übertragen — Phase 6 — Status: Fertig
- [x] Ebenen-Mischmodi pro Maske — Phase 6 — Status: Fertig — alle fünf Modi (Normal/Multiplizieren/Weiches Licht/Farbe/Luminanz), CPU-only (siehe ADR-0032 Punkt 4/Schritt 11 zur GPU-Zurückstellung)
- [x] Masken als wiederverwendbare Bausteine speichern — Phase 6 — Status: Fertig (abweichend, siehe ADR-0032 Punkt 6) — bewusst nur clientseitig/session-lokal statt über die Presets-Katalog-Infrastruktur (Ordner/Versionen/SQLite) aus Phase 5; ein katalogseitiges Pendant wäre dieselbe Größenordnung an Aufwand wie das gesamte Presets-System
- [x] Maskenkette mit Drag-&-Drop-Sortierung — Phase 6 — Status: Fertig — die Reihenfolge ist zugleich die Anwendungsreihenfolge in der Pipeline, nicht nur die Anzeige

## 3.4 Modul ENTWICKELN — Workflow

- [x] Undo/Redo (unbegrenzt, dauerhaft über `edit_history`, überlebt App-Neustart) — Phase 2 — Status: Fertig — **Teil-Einschränkung:** kein sichtbares, klickbares Verlaufs-*Panel* mit benannten Schritten (nur Rückgängig/Wiederholen um je einen Schritt) — siehe `DECISIONS.md` ADR-0018s Korrektur-Notiz; die Backend-Grundlage (`list_edit_history`) für ein solches Panel existiert noch nicht und ist ein möglicher Ausbau für eine spätere Phase
- [x] Schnappschüsse — Phase 6 — Status: Fertig — eigene `snapshots`-Tabelle mit unabhängiger EDL-Kopie je Schnappschuss statt eines Verweises auf einen `edit_history`-Stand (Korrektur ggü. der ursprünglichen Plan-Formulierung, siehe `DECISIONS.md` ADR-0032 Punkt 6 Nachtrag)
- [x] Vorher/Nachher in vier Ansichten — Phase 6 — Status: Fertig — **Teil-Einschränkung:** die Trennlinie der geteilten Modi sitzt fest bei 50 % (kein ziehbarer Regler), siehe `PLAN.md` Schritt 8
- [x] Einstellungen kopieren/einfügen (granular) — Phase 6 — Status: Fertig
- [x] Vorherige übernehmen — Phase 6 — Status: Fertig
- [x] Synchronisieren über beliebig viele Bilder — Phase 6 — Status: Fertig
- [x] Auto-Sync-Modus — Phase 6 — Status: Fertig (abweichend) — überträgt bewusst immer alle EDL-Sektionen statt der für den manuellen Sync-Knopf verfügbaren granularen Auswahl (siehe `PLAN.md` Schritt 9)
- [x] Referenzansicht — Phase 6 — Status: Fertig — Referenzbild links (statisch, letzter committeter Stand eines frei wählbaren anderen Fotos) und Arbeitsbild rechts, unabhängiger Zoom/Pan je Hälfte
- [x] Soft-Proof (Zielprofil, Renderpriorität, Farbumfangswarnung, Papierweiß) — Phase 6, echtes ICC-Farbmanagement seit Phase 12 Schritt 6 — Status: Fertig — echter `lcms2::Transform::new_proofing`-Transform (`SOFT_PROOFING`/`GAMUT_CHECK`, `cmsSetAlarmCodes`) über dieselbe `develop/...`-Route wie die normale Vorschau, gegen die vier gebündelten Standardprofile ODER eine frei wählbare `.icc`-Datei (`DECISIONS.md` ADR-0039-Nachtrag II); nur die Papierweiß-Simulation bleibt eine kleine clientseitige Tonwertkompression, da `lcms2` dafür keine eingebaute Entsprechung hat

## 3.5 PRESET- UND TEMPLATE-SYSTEM
<!-- Scope-Präzisierung siehe DECISIONS.md ADR-0031: Preset-Grundlagen
     (erste 9 Zeilen unten, bedingte Presets vereinfacht auf UND-verknüpfte
     Regeln über eine feste Metadatenfeld-Liste statt einer freien
     Bedingungssprache) sind die Phase-5-Basis. Preset-Generator (KI) auf
     Phase 7 verschoben (bereits in ARCHITECTURE.md §7 so vorgesehen).
     Adobe-Interop auf eine spätere Phase verschoben (dasselbe
     Format-Problem wie beim Objektivprofil-/DCP-Import, ADR-0028).
     Templates-Unterabschnitt: nur Import-/Umbenennungs-Templates ziehen
     nach Phase 5 vor (Rust-Unterbau existiert bereits unbenutzt aus
     Phase 3, war dort fälschlich als erledigt-fällig getaggt), alle
     anderen bleiben auf der Phase ihres zugehörigen Subsystems (Export-
     Engine: Phase 8–9). -->

- [x] Presets: wählbare EDL-Teilmenge, Checkbox-Dialog beim Speichern — Phase 5 — Status: Fertig
- [x] Preset-Ordnerhierarchie, Drag & Drop, Favoriten, Suche, Tags — Phase 5 — Status: Fertig (abweichend, siehe ADR-0031: Verschieben zwischen Ordnern per Dropdown-Auswahl statt Drag & Drop — dieselbe funktionale Fähigkeit, andere Interaktion; kein separates Suchfeld, Presets werden nach Ordner gefiltert angezeigt)
- [x] Preset-Stärke 0–200 %, nachträglich änderbar — Phase 5 — Status: Fertig
- [x] Live-Vorschau (Hover im Bild + Thumbnail in der Liste) — Phase 5 — Status: Fertig
- [x] Preset-Stapel mit editierbarer Reihenfolge — Phase 5 — Status: Fertig
- [x] Bedingte Presets (Bedingungssprache im UI-Builder) — Phase 5, echter UND/ODER-Regelbaum seit Phase 13 Schritt 7 — Status: Fertig — feste Feldliste (ISO/Blende/Brennweite/Kameramodell/Objektiv, ADR-0031) bleibt, aber seit Phase 13 Schritt 7 mit echter Verschachtelung/ODER statt nur flacher UND-Verknüpfung (`RuleTreeEditor.tsx`, `DECISIONS.md` ADR-0040-Nachtrag V); alte Presets werden beim Laden automatisch migriert
- [x] Import/Export `.apx` — Phase 5 — Status: Fertig
- [x] Import/Export Adobe `.xmp` (beide Richtungen) — Phase 9 — Status: Fertig (siehe PLAN.md Schritt 2, `apx_export::xmp`) — Zeile war fälschlich noch als „Nicht begonnen" stehen geblieben, obwohl der `.xmp`-Teil (Adobe `crs:`-Entwickeln-Einstellungen, Basic+HSL) seit Phase 9 Schritt 2 echt bidirektional implementiert und bis ins Frontend verdrahtet ist — Korrektur bei der Phase-11-Bestandsaufnahme, siehe DECISIONS.md ADR-0038
- [x] Export Adobe `.lrtemplate` — Phase 11 Schritt 8 — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0038) — `apx_export::lrtemplate::generate_lrtemplate`: Lua-Tabellenzuweisung (`s = { ... }`), Struktur anhand einer real abgerufenen Beispiel-Vorlagendatei rekonstruiert, deckt dieselbe Basic+HSL-Teilmenge ab wie der `.xmp`-Crs-Export. Nur Export (kein Import), wie geplant — Import bräuchte einen robusten Lua-Parser für ein nicht spezifiziertes Format
- [x] Preset-Versionierung mit Diff-Ansicht — Phase 5 — Status: Fertig
- [x] Preset-Generator per LLM (natürlichsprachliche Beschreibung) — Phase 7 — Status: Fertig — echter Anthropic-Messages-API-Aufruf (`apx-ai::preset_generator::generate_from_llm`), serverseitig validiert (Antwort wird rekursiv auf ein neutrales EDL gemergt und muss vollständig deserialisierbar sein), Anthropic-API-Schlüssel vom Nutzer selbst hinterlegt (kein mitgelieferter Schlüssel). **Zusätzlich ohne API-Schlüssel nutzbar:** „Prompt für Claude-App" kopiert denselben Prompt in die Zwischenablage zum Einfügen in die kostenlose Claude-App (claude.ai); die Antwort von dort lässt sich per „Antwort aus der Claude-App einfügen" zurück einfügen und wird serverseitig genauso validiert wie der direkte API-Aufruf — kein Netzwerk-Aufruf, keine Kosten
- [x] Referenzbild-Modus (numerische Optimierung, kein LLM) — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 4) — Koordinatenabstieg über die sechs tonwertbezogenen Grundeinstellungs-Parameter, Histogramm-Distanz (Kumulativsummen/Earth-Mover's) als Zielfunktion statt eines vollständigen Gradientenverfahrens über alle Regler
- [x] Variationen-Generator (Kontaktbogen) — Phase 7 — Status: Fertig — deterministisch geseedete kleine Störungen eines Basis-Presets, reproduzierbar über denselben Seed
- [x] Preset aus Bearbeitung lernen (Mustererkennung über mehrere Bilder) — Phase 7 — Status: Fertig (abweichend, siehe ADR-0033 Punkt 4) — arithmetisches Mittel der committeten EDL-Werte je Sektion über die ausgewählten Fotos statt echter Mustererkennung; strukturierte Listen (Kurvenpunkte, Farbmischer-Regionen) werden vom ersten Foto übernommen statt zusammengeführt
- [x] Export-Templates (Ziel, Format, Qualität, Farbraum, Größe, Schärfung, Metadaten, Wasserzeichen, Mehrfachziel) — Phase 8, Mehrfachziel Phase 12 Schritt 5 — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0039) — eine generische `templates`-Tabelle (`kind="export"`) speichert das komplette `ExportPhotoOptions`-JSON als benannte Vorlage; **Mehrfachziel** (ein Export-Vorgang, mehrere Zielordner/-formate gleichzeitig): `ExportDialog.tsx` erlaubt "+ Weiteres Ziel hinzufügen" (merkt sich eine Momentaufnahme aus Zielordner + allen aktuellen Optionen), "Alle N Ziele exportieren" reicht die Fotos an jedes gemerkte Ziel weiter — reine Frontend-Schleife über den bereits bestehenden `enqueue_export_photo`-Befehl/die Export-Warteschlange, kein neuer Backend-Mechanismus nötig
- [x] Wasserzeichen-Templates — Phase 8 — Status: Fertig (abweichend) — Wasserzeichen-Felder sind Teil von `ExportPhotoOptions` und damit bereits in den Export-Vorlagen enthalten, kein eigener Vorlagentyp
- [x] Metadaten-Templates (Copyright/Ersteller/Kontakt/IPTC) — Phase 8 — Status: Fertig (abweichend) — ebenso Teil von `ExportPhotoOptions`/Export-Vorlagen statt eines eigenen Vorlagentyps
- [x] Import-Templates — Phase 5 — Status: Fertig (vorgezogen aus Phase 3, siehe ADR-0031 Punkt 7 — Rust-Unterbau existierte bereits seit Phase 3 unbenutzt, Frontend-Anbindung jetzt in Schritt 9 nachgezogen)
- [x] Umbenennungs-Templates mit Token-Editor — Phase 5 — Status: Fertig (vorgezogen aus Phase 3, siehe ADR-0031 Punkt 7)
- [x] Layout-Templates (Druck/Buch/Diashow/Web) — Phase 8 — Status: Fertig (abweichend, siehe PLAN.md Schritt 8) — dieselbe generische `templates`-Tabelle (`kind="print"/"book"/"slideshow"/"web"`); Anlegen läuft über eingefügtes JSON im `TemplatesDialog`, noch kein „Aktuelle Einstellungen speichern"-Knopf direkt in den vier Dialogen
- [x] Workflow-Templates (Import→Filter→Preset→Export als ein Klick) — Phase 8 — Status: Fertig (abweichend, siehe PLAN.md Schritt 8) — echte Ein-Klick-Ausführung (Preset-EDL mischen, committen, exportieren, pro ausgewähltem Foto); ohne den Import-/Filter-Schritt aus der ursprünglichen Formulierung — läuft auf der bereits getroffenen Fotoauswahl, wie alle übrigen Phase-8-Exportdialoge
- [x] Template-Marktplatz-Struktur (lokales Repo-Format, Manifest, Installation) — Phase 8 — Status: Fertig (abweichend, siehe PLAN.md Schritt 8) — `.apxt`-Dateiformat mit Manifest (`schema_version`/`kind`/`name`), Export/Import über Tauri-Dateidialoge; kein Online-Marktplatz-Hosting, wie geplant

## 3.6 Weitere Module

### Karte
- [x] GPS aus EXIF, Kartenansicht — Phase 8 — Status: Fertig — GPS-Lesepfad war schon vorhanden (`apx_raw::metadata`), neu ist `MapView.tsx` (Leaflet + OpenStreetMap-Kacheln, einzige Netzwerk-Abhängigkeit dieser Phase) mit einem Marker je geotaggtem Foto
- [x] GPX-Tracklog-Import — Phase 8 — Status: Fertig — `apx_export::map::parse_gpx` (nur `<trkpt>`, Streaming-Parser), als eigene Linie auf der Karte überlagerbar
- [x] Fotos per Drag auf Karte setzen — Phase 8 — Status: Fertig (abweichend, siehe PLAN.md Schritt 7) — Klick-Platzieren-Modus statt echtem HTML5-Drag-and-drop: Standort-Knopf am ausgewählten Foto aktivieren, dann setzt ein Kartenklick die Koordinate
- [x] Ortsschlagworte automatisch (Reverse Geocoding) — Phase 8 — Status: Fertig (abweichend, siehe PLAN.md Schritt 7) — Reverse-Geocoding ist vollständig offline umgesetzt (`apx_export::map::reverse_geocode`) und zeigt den Ortsnamen im Marker-Popup an; es schreibt noch **keine** Schlagworte automatisch ins Foto (kein Aufruf von `add_photo_keyword` aus der Kartenansicht heraus)
- [x] Reiserouten-Ansicht — Phase 8 — Status: Fertig — gestrichelte Linie verbindet alle geotaggten Fotos in der ohnehin nach Aufnahmezeit sortierten Reihenfolge (`list_geotagged_photos`)

### Buch
- [x] Seitenlayouts, Vorlagen, Text-Stile — Phase 8 — Status: Fertig (abweichend, siehe PLAN.md Schritt 5) — fünf feste Seitenvorlagen (`apx_export::book::PageTemplate`) statt einer frei konfigurierbaren Slot-Engine, ein Text-„Stil" (eine vom Nutzer gewählte Schriftdatei, wie bei den Diashow-Titelkarten) statt wählbarer Schriftfamilien
- [x] Automatische Befüllung — Phase 8 — Status: Fertig — `auto_fill_pages` verteilt die Fotoauswahl reihum auf Seiten, Bildunterschriften sind automatisch der Dateiname
- [x] PDF-Export — Phase 8 — Status: Fertig — `printpdf`, bewusst ohne dessen Standard-Features (kein `html`/`azul-layout`, keine eigene Bilddekodierung) — jede Seite ist bereits ein fertig komponiertes Bild, wird direkt als `RawImage` eingebettet
- [x] Druckerei-Presets — Phase 8 — Status: Fertig (abweichend) — drei feste Parametersätze (Beschnitt/Auflösung/Hintergrund), keine anbieterspezifische Validierung

### Diashow
- [x] Übergänge, Ken-Burns-Effekt — Phase 8 — Status: Fertig (abweichend, siehe ADR-0034/PLAN.md Schritt 4) — nur harter Schnitt/Überblendung (kein Wipe/Slide), Live-Wiedergabe komplett im Frontend-`<canvas>` (`lib/slideshow.ts`, `SlideshowPlayer.tsx`)
- [x] Musik-Synchronisation — Phase 8 — Status: Fertig — Tauri-Webview-`<audio>`-Element über eine neue `apx://music/<pfad>`-Protokollroute, kein Rust-Audio-Crate
- [x] Intro/Outro-Screens — Phase 8 — Status: Fertig — Text auf Farbfläche, wiederverwendet `watermark::apply_text_watermark`s Glyph-Rasterisierung statt eines zweiten Textpfads
- [x] Video-Export (MP4 via ffmpeg) — Phase 8 — Status: Fertig — neues `apx_export::video`-Modul rendert Frame für Frame (Ken-Burns-Zuschnitt inkl. Seitenverhältnis-Korrektur, Überblendungs-Mischung) und pipet sie roh in ein System-`ffmpeg`; `ffmpeg_available()` prüft vorab, sonst klare Fehlermeldung statt eines gebündelten Binaries

### Drucken
- [x] Einzelbild, Kontaktbogen, Bilderpaket, benutzerdefiniertes Raster — Phase 8 — Status: Fertig (abweichend, siehe ADR-0034/PLAN.md Schritt 3) — Bilderpaket nutzt drei feste Vorlagen statt echtem Bin-Packing beliebiger Formate
- [x] Randeinstellungen, Zellen, Zoom — Phase 8 — Status: Fertig — Rand/Zellabstand in Zoll, Zoom als Einpassen/Füllen-Beschneiden
- [x] Druckschärfung, Farbmanagement, Druckauflösung — Phase 8 — Status: Fertig — nutzt die Export-Engine aus Schritt 1/2 (Unsharp-Masking, ICC-Profile) je Foto vor der Seitenkomposition; Druckauflösung als DPI-Parameter zusammen mit den Seitenmaßen
- [x] Speichern als JPEG — Phase 8 — Status: Fertig — kein System-Druckertreiber-Zugriff, Ausgabe ist eine druckfertige JPEG-Datei über einen Speichern-unter-Dialog

### Web
- [x] HTML-/responsive Galerie-Generator, Themes — Phase 8 — Status: Fertig — `apx_export::web`, eine statische HTML-Datei mit eingebettetem CSS für drei Themes (Hell/Dunkel/Minimal), Fotos als JPEG-Miniaturbilder über denselben Render-Pfad wie Druck/Buch
- [x] Upload via FTP/SFTP — Phase 8 — Status: Fertig (abweichend, siehe ADR-0034) — echtes FTP/FTPS (`suppaftp`) und echtes SFTP (`russh`/`russh-sftp`, reines Rust); SFTP nimmt jeden Server-Schlüssel an statt eines Known-Hosts-Abgleichs (`AcceptAnyHostKey`)

### Export
- [x] Formate JPEG/PNG/TIFF/PSD/DNG/WebP/AVIF/HEIF/JPEG XL — Phase 8, PSD/JPEG-XL/DNG in Phase 11 nachgerüstet — Status: Fertig (abweichend, siehe ADR-0034 Punkt 1 und ADR-0038) — JPEG/PNG/TIFF/WebP(verlustfrei)/AVIF (`apx_export::format`), PSD (flach, `ag-psd`, Phase 11 Schritt 2), JPEG XL (`gamut-jxl`, Encoder bindet libjxl (C) via `gamut-jxl-sys`/`cmake`, Decoder ist reines Rust, Phase 11 Schritt 2) und eine DNG-Konvertierung beim Import ("Linear DNG", `gamut-dng`, Phase 11 Schritt 1) sind echt umgesetzt; nur HEIF bleibt zurückgestellt — die einzige real geprüfte Crate (`heif` 0.1.0) ist eine Registry-Fassade (unveränderter `cargo new`-Vorlagentext trotz irreführender Beschreibung), die einzige echte Alternative (`heif-rs`) zieht 190+ transitive Abhängigkeiten (`bindgen`/`libclang`) und war für das Plattenkontingent dieser Sandbox zu riskant, siehe ADR-0038
- [x] Farbräume sRGB/AdobeRGB/ProPhoto/Display-P3/eigenes ICC — Phase 8 — Status: Fertig (abweichend, siehe ADR-0034) — `apx_export::icc`, vier Standardprofile aus Chromatizitätswerten aufgebaut (`lcms2`) statt als `.icc`-Dateien mitgeliefert; ProPhoto/Display-P3 nutzen eine vereinfachte Potenzgammakurve statt der echten stückweisen Übertragungsfunktion
- [x] Bit-Tiefe 8/16 — Phase 8 — Status: Fertig (abweichend, siehe ADR-0034 Punkt 1) — 16-Bit ist für PNG/TIFF eine lineare Streckung des fertigen 8-Bit-Werts auf den vollen 16-Bit-Bereich (Dateiformat-Kompatibilität), keine echte zusätzliche Tonwertpräzision — dafür müsste `apx_pipeline::develop::render_rgba8` durchgehend auf einem `u16`/`f32`-Pfad rendern, siehe `apx_export::format`s Moduldoku
- [x] Größenbegrenzung (Kante/Megapixel/Dateigröße) — Phase 8 — Status: Fertig (`apx_export::resize`, Dateigröße per iterativer JPEG-Qualitätssuche)
- [x] Ausgabeschärfung nach Medium — Phase 8 — Status: Fertig (`apx_export::sharpen`, Unsharp-Masking mit Bildschirm-/Matt-/Hochglanz-Voreinstellungen)
- [x] Wasserzeichen, Metadaten-Filter — Phase 8 — Status: Fertig (abweichend, siehe ADR-0034) — Bild-/Text-Wasserzeichen (`apx_export::watermark`, Text braucht eine vom Nutzer gewählte Schriftdatei statt einer eingebetteten); Metadaten-Filter als echter minimaler EXIF-Writer (`apx_export::metadata`), **nur für JPEG**, GPS/`DateTimeOriginal`-Sub-IFD zurückgestellt
- [x] Export-Warteschlange (Fortschritt, Pausieren, Priorisieren) — Phase 8 — Status: Fertig (abweichend, siehe ADR-0034) — echte `apx_export::queue::ExportQueue` (Priorisierung, Pausieren, Abbrechen) mit einem Hintergrund-Worker in `apx-app`; Fortschritt per Abfragen (150ms Worker, 250ms Frontend) statt Weck-Benachrichtigung/Tauri-Events, keine Persistenz über App-Neustarts hinweg

### Zusätzliche Module (über Lightroom hinaus)
- [x] Node-Editor (Pipeline als Knotengraph) — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 7) — kein `@xyflow/react`-Graph-Canvas (würde eine frei umbaubare Topologie vortäuschen, die es nicht gibt): eine geordnete Liste über die feste `develop::render_rgba8`-Reihenfolge, ein Eintrag je Stufe mit Ein/Aus-Schalter (`EdlV4::stage_enabled`) und Sprung zum zugehörigen Regler-Abschnitt
- [x] Stapelverarbeitungs-Konsole (Vorschau, Trockenlauf, Rückgängig) — Phase 9/11 — Status: Fertig (siehe PLAN.md Phase 11 Schritt 9, DECISIONS.md ADR-0038) — war in ADR-0035s Scope-Aufzählung, bekam aber nie einen eigenen Bauschritt zugeteilt (Lücke im Schritt-0-Zuschnitt, bei der Schritt-12-Abnahme aufgefallen, siehe ADR-0036); jetzt echtes feldübergreifendes Batch-Journal (`batch_operations`/`batch_operation_items`), `apx_catalog::repository::batch` mit `preview_batch_rule`/`apply_batch_rule`/`undo_batch_operation`, wiederverwendet die bestehende `FilterCriteria` für die Auswahl; Aktionen: Bewertung setzen, Farbmarkierung setzen, Schlagwort hinzufügen; Undo liest das Journal rückwärts und löscht es danach (zweites Undo desselben Stapels ist ein sicheres No-op)
- [x] Fokus-Stacking — Phase 9 — Status: Fertig (siehe PLAN.md Schritt 8) — Laplacian-Schärfemaß je Pixel, schärfste Quelle gewinnt; setzt bereits ausgerichtete Aufnahmen voraus (keine eigene Registrierung)
- [x] HDR-Zusammenführung — Phase 9 — Status: Fertig (siehe PLAN.md Schritt 8) — Debevec-artige gewichtete Fusion nach EXIF-Belichtungszeit im linearen Raum + Reinhard-Tonemap; setzt bereits ausgerichtete Aufnahmen voraus
- [x] Panorama-Zusammenführung — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 8) — **v1 nur Verschiebungs-Registrierung** per 2D-Phasenkorrelation (`rustfft`), kein sphärisch/zylindrisch/perspektivisches Homographie-Stitching für Freihandaufnahmen (`opencv` bräuchte eine fehlende Systembibliothek), kein Auto-Crop/-Fill
- [x] Astro-Stacking — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 8) — Sigma-geclipptes Mittel über Kurzbelichtungen, registriert per derselben Phasenkorrelation wie Panorama; echte Sternzentroid-/Dreiecks-Registrierung zurückgestellt
- [x] Tethered Shooting (gphoto2/PTP, Live-View, Auto-Preset) — Phase 9/11 — Status: Fertig (abweichend, siehe PLAN.md Schritt 11 und Phase 11 Schritt 10) — **kein Live-View** (nicht Teil des in `PLAN.md` beschriebenen Kernablaufs Kamera erkennen → auslösen → herunterladen → Auto-Preset); `Gphoto2Backend`s `libgphoto2`-FFI-Aufrufe kompilieren und laufen jetzt real (Phase 11 Schritt 10: `libgphoto2-dev` real installiert, Linux-CI-Zweig baut/testet zusätzlich mit `--features tethering`, ein neuer Test ruft `detect_camera()` echt gegen die Bibliothek auf und erwartet ohne angeschlossene Kamera ein sauberes `Ok(None)`) — **bleibt ehrlich begrenzt**: echte Aufnahme/echter Download bleiben ungetestet (keine physische Kamera verfügbar); ohne das Cargo-Feature `tethering` (weiterhin Standard auf macOS/Windows-CI) läuft ausschließlich die klar als Simulation markierte `FakeBackend`
- [x] Vergleichs-Grid (bis 9 Versionen, sync. Zoom) — Phase 9 — Status: Fertig (siehe PLAN.md Schritt 7) — reused `CompareGridView.tsx` (Phase 9 Schritt 3) mit Quelle+virtuellen Kopien statt eines zweiten Rendering-Pfads; synchronisierter Zoom über einen gemeinsamen Skalierungsfaktor (vier feste Stufen), echtes Pan-Sync zurückgestellt (siehe PLAN.md)
- [x] Skript-API (Lua/Rhai) + Plugin-System mit stabilem ABI — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 9) — Rhai statt Lua (reines Rust, sandboxbar, kein C-Interpreter nötig); Plugin-ABI ehrlich begrenzt (ADR-0035 Punkt 3): versionierte, geprüfte Kompatibilität für **einen** festen Erweiterungspunkt (Custom-Effekt), keine Zusage unbegrenzter künftiger Binärkompatibilität
- [x] Zeitleisten-Ansicht der Bearbeitungshistorie — Phase 9 — Status: Fertig (siehe PLAN.md Schritt 7) — `HistoryTimelineDialog.tsx`, Punktposition proportional zu `created_at`, Klick springt per neuem `goto_develop_edit`/`repository::edits::goto` direkt zum Stand
- [x] Verlaufs-Vergleich (zwei Schritte gegenüberstellen) — Phase 9 — Status: Fertig (siehe PLAN.md Schritt 7) — dasselbe Diff-Muster wie `PresetVersionsDialog.tsx` (Phase 5 Schritt 8), derselbe Sektionsumfang wie das Presets-System
- [x] Kollaborationsmodus (Katalog-Teilfreigabe, Merge, Konfliktauflösung) — Phase 9 — Status: Fertig (abweichend, siehe PLAN.md Schritt 10) — asynchroner Export→Weitergabe→Import→Konfliktauflösung-Ablauf über `.apxs`-Dateien (keine Pixel-Bytes, Matching per `content_hash`), kein Echtzeit-Mehrbenutzer-Modus (ADR-0035 Punkt 4)
- [x] Barrierefreiheit (Tastatur, Screenreader, Kontrastmodus, UI-Skalierung 75–200 %) — Phase 10 — Status: Fertig (abweichend, siehe PLAN.md Schritt 6/ARCHITECTURE.md §14) — Kontrastmodus/UI-Skalierung/reduzierte Bewegung app-weit über `uiSettings` verdrahtet (`index.css`-Token-Override + `App.tsx`-Effekt); Fokus-Falle (`lib/a11y.ts::useFocusTrap`) nur als Muster auf die zwei in dieser Phase neuen Dialoge angewendet, nicht auf die älteren, bereits e2e-getesteten Dialoge ausgerollt; ARIA-Sweep nur punktuell (neue Dialoge + `role="dialog"`/`aria-label`), kein vollständiger 43-Komponenten-Durchgang

## 4. UI-Anforderungen

- [x] Grundlayout (Kopfzeile, linke Spalte, Mitte-Viewer, unten Filmstreifen) — Phase 1 — Status: Fertig
- [x] Rechte Werkzeug-Palette, Modul-Umschalter oben — Phase 3 — Status: Fertig (abweichend, in Phase 10 Schritt 2 umgesetzt, siehe DECISIONS.md ADR-0037) — `Header.tsx` in zwei Zeilen: Ansichts-Umschalter (Raster/Karte/Info/Entwickeln) oben, übrige Module in vier benannten Gruppen (Ausgabe/Vorlagen & Organisation/Fortgeschritten/Analyse) statt einer flachen ~20-Knopf-Liste; kein Lightroom-artiger vollständiger Bildschirmwechsel pro Modul — jeder Knopf öffnet unverändert denselben Dialog wie zuvor, nur sichtbar gruppiert. `DevelopPanel`/`MasksPanel` unter gemeinsamer visueller Außenhülle als rechte Werkzeug-Palette
- [x] Dark-First-Theme — Phase 1 — Status: Fertig (Tailwind-CSS-4-`@theme`-Tokens in `src/index.css`)
- [x] Vollständiges helles Theme + benutzerdefinierte Themes (Design-Tokens) — Phase 10 — Status: Fertig (abweichend, siehe PLAN.md Schritt 7) — zweiter `@theme`-Tokensatz unter `[data-theme="light"]`; „benutzerdefiniert" ist Dark/Hell + eine einstellbare Akzentfarbe (`--color-accent`-Override), keine vollständige Theme-Editor-Engine mit beliebig vielen speicherbaren Themes
- [x] Paletten ein-/ausklappbar, breitenziehbar, Arbeitsbereich-Preset speicherbar — Phase 3/11 — Status: Fertig (abweichend, in Phase 10 Schritt 3 umgesetzt, Phase 11 Schritt 11 vervollständigt) — `PaletteFrame.tsx` (Ziehgriff 75–480px, Einklapp-Knopf, `localStorage`) jetzt auf allen sechs Paletten (Sidebar/Presets/Metadaten-Panel/**Entwickeln/Masken**); „Arbeitsbereich zurücksetzen" im `SettingsDialog.tsx`. Direkt nach der `DevelopPanel`/`MasksPanel`-Umstellung volle `develop-flow.spec.ts`/`masks-flow.spec.ts`-Regression gefahren (40 Tests grün) — dieselbe Vorsicht, die Phase 10 Schritt 3 zurückgestellt hatte, jetzt mit Testnetz.
- [x] Regler-Standardverhalten (Doppelklick=Reset, Direkteingabe, Pfeiltasten=Feinschritt, Shift=Grobschritt) — Phase 2 — Status: Fertig — **Teil-Einschränkung:** Alt-Maskenvorschau nicht implementiert (keiner der 7 Phase-2-Regler hat eine Maskenvorschau — die betrifft eher spätere Regler wie Schärfung/lokale Masken, siehe Phase 4/6)
- [x] Grundlegende Tastenkürzel (Bildwechsel, Zoom, Vollbild) — Phase 1 — Status: Fertig
- [x] Vollständig belegbare Tastenkürzel + Cheatsheet-Overlay (`?`) — Phase 10/11 — Status: Fertig (abweichend, siehe PLAN.md Schritt 5 und Phase 11 Schritt 11) — `lib/keybindings.ts`: zentrale, umbelegbare Zuordnungstabelle für die globalen `App.tsx`-Kürzel (Bildwechsel/Palette/Undo-Redo/Vollbild/Flaggen/Cheatsheet) **und jetzt zusätzlich** `Viewer.tsx`s Zoom-Zifferntasten (`zoom-fit`/`zoom-100`) sowie `DevelopPanel.tsx`s eigener Ctrl/Cmd+Z-Handler (nutzt dieselben `undo`/`redo`-IDs wie die Bibliotheks-Metadaten, da sich beide Kontexte gegenseitig ausschließen); Neubelegung in `localStorage`. **Verbleibende, bewusste Einschränkung:** Bewertungs-Zifferntasten 0–5 (parametrisierte Ziffernreihe) und Kurven-/Masken-Editoren mit `role="slider"`-Pfeiltasten-Feinjustierung bleiben fest — die am dichtesten getesteten Pfade im Frontend, siehe `lib/keybindings.ts`s Moduldoku
- [x] Befehlspalette `Strg/Cmd+K` — Grundgerüst (Ordner/Befehle) — Phase 1 — Status: Fertig
- [x] Befehlspalette — vollständig (jede Funktion/jedes Preset) — Phase 5 — Status: Fertig (abweichend, in Phase 10 Schritt 4 umgesetzt) — alle Header-Funktionen (inkl. der neun weiterhin `Header.tsx`-lokalen Dialoge über eine neue `pendingCommand`-Store-Brücke), alle Presets (wendet an), alle Fotos der aktuellen Ansicht (wählt aus), Ordner (bestehend)
- [x] Nicht-blockierende UI für alle langen Operationen (Hintergrund, Fortschritt, Abbruch) — Phase 1 (Import) / laufend erweitert — Status: Fertig für Import; wird bei jeder neuen langen Operation (Export, Stapelverarbeitung, …) fortgeführt

## 5. Phase 10 — Politur (SPEC.md §5, bisher nur als Prosa-Satz, nicht als eigene Zeilen erfasst — analog zur bei der Phase-9-Abnahme gefundenen Stapelverarbeitungs-Konsole-Lücke, ADR-0036, hier bei der Phase-10-Abnahme nachgetragen)

- [x] Performance-Profiling gegen die Ziele aus SPEC.md §2.4 — Phase 10 — Status: Fertig (abweichend, siehe PLAN.md Schritt 10) — Regler-Latenz/Bildwechsel bereits in Phase 2 Schritt 7 real gemessen (auf dieser Sandbox klar verfehlt, ehrlich dokumentiert), 100k-Raster bereits in Phase 3 verifiziert; Import-Zeit/Idle-Speicher in dieser Sandbox strukturell nicht neu messbar (keine 1000 echten RAWs, kein nativer Tauri-Runtime-Host) — für die Import-Zeit immerhin ein konkret benannter Code-Befund (sequenzielle Verarbeitung ohne `rayon` in `apx-app/src/import/mod.rs`) statt einer bloßen Lücke, bewusst nicht in diesem Schritt behoben (Kernlogik aus Phase 1, dicht getestet)
- [x] Lokalisierung (Deutsch und Englisch) — Phase 10/11 — Status: Fertig (abweichend, siehe PLAN.md Schritt 8 und Phase 11 Schritt 11) — `lib/i18n.ts` + `lib/locales/de.ts`/`en.ts`, Header/Sidebar/Presets-/Metadaten-Panel/Einstellungen/Cheatsheet **und jetzt alle 13 Dialog-Komponenten** (Export/Druck/Diashow/Buch/Web/Vorlagen/Organisieren/Stacking/Skript & Plugins/Kollaboration/Tethering/Metadaten-Editor/Statistik) übersetzt. **Verbleibende Teil-Einschränkung:** `SlideshowPlayer.tsx` (die separate Vollbild-Wiedergabekomponente) und die von `MetadataDialog.tsx`/`SavePresetDialog.tsx` gemeinsam genutzten `PRESET_CONDITION_*`-Labels aus `lib/presets.ts` bleiben unübersetzt — offene Ausbaustufe
- [x] Onboarding — Phase 10 — Status: Fertig (siehe PLAN.md Schritt 9) — `OnboardingDialog.tsx`, einmaliges automatisches Erstanzeigen über `uiSettings.onboarding_seen`, jederzeit erneut über die Befehlspalette aufrufbar
- [x] Installer und Signierung für alle drei Plattformen — Phase 10/11 — Status: Fertig (abweichend, siehe PLAN.md Schritt 11 und Phase 11 Schritt 11) — `@tauri-apps/cli` + neuer CI-`release`-Job auf dem bestehenden 3-OS-Matrix (nur bei Tag-Push/`workflow_dispatch`), Signierungskonfiguration strukturell vorbereitet und konditional auf GitHub-Secrets (macOS: Zertifikat+Notarisierung direkt aus der Umgebung gelesen; Windows: PFX-Import + Fingerabdruck-Config-Override) — überspringt sich selbst ohne Fehlschlag, wenn Secrets fehlen. Phase 11 Schritt 11 hat den betriebssystemunabhängigen Teil der Mechanik lokal nachgewiesen (selbstsigniertes Test-Zertifikat per `openssl` erzeugt, Base64-Rundreise byte-identisch bestätigt, SHA1-Fingerabdruck ermittelt — genau die Schritte, die `ci.yml`s PowerShell-Import vor dem eigentlichen `Import-PfxCertificate`-Aufruf durchläuft). **Ehrlich begrenzt:** kein echtes Zertifikat/Apple-Developer-Konto beschaffbar; ein echter `Import-PfxCertificate`-Lauf in einen Windows-Zertifikatspeicher und das Setzen eines echten GitHub-Actions-Secrets liegen außerhalb der Werkzeuge/Ausführungsumgebung dieser Sitzung (keine Windows-Runner, kein Secret-Schreibzugriff) — erzeugte Installer bleiben bis zur Hinterlegung eigener Secrets durch den Nutzer unsigniert

## Alleinstellungsmerkmale (Phase 14 — jenseits von Lightroom)

Zehn eigenständige Fähigkeiten ohne Lightroom-Entsprechung (siehe `DECISIONS.md` ADR-0041 für die Recherche-Belege je Punkt).

- [x] KI-Ausfüllen über Bildränder hinaus (Canvas-Erweiterung/Outpainting) — Phase 14 Schritt 1 — Status: Fertig — dieselbe LaMa-Inpainting-Inferenz wie der Reparaturpinsel, nur mit einer bis zum neuen Rand erweiterten Maske
- [x] Frequenztrennung für Präzisions-Retusche — Phase 14 Schritt 2 — Status: Fertig — Tief-/Hochfrequenz-Zerlegung, Klon-/Ausbesserstriche wirken wahlweise nur auf eine Ebene, plus ein reiner Anzeige-Modus im Viewer
- [x] Mehrfachbelichtung & Layer-Blend-Modi — Phase 14 Schritt 3 — Status: Fertig — beliebig viele Ebenen (Katalog-Foto oder Textur) mit Blend-Modus/Deckkraft/Skalierung/Versatz übereinandergelegt
- [x] Echte Halation-/Bloom-Simulation — Phase 14 Schritt 4 — Status: Fertig — Lichter-Maske → Einfärbung → Weichzeichnung → Screen-Rückmischung, Radius/Betrag/Farbton regelbar
- [x] Automatischer Stil-Konsistenz-Check fürs Shooting — Phase 14 Schritt 5 — Status: Fertig — Lab-Signatur je Foto, Ausreißer-Erkennung über eine Fotomenge, Angleichungs-Vorschlag per Weißabgleich/Belichtung
- [x] Vektorskop + Wellenform-Monitor — Phase 14 Schritt 6 — Status: Fertig — reine Frontend-Analyse neben dem bestehenden Histogramm, kein neuer Backend-Command
- [x] Farb-Harmonie-Rad — Phase 14 Schritt 7 — Status: Fertig — k-means-Paletten-Extraktion (CIE-Lab), Komplementär-/Triade-/Split-Komplementär-/Analog-Harmonie, "Harmonisieren" verschiebt die HSL-Farbton-Regler
- [x] KI-Tiefenschärfe-Simulator "Virtuelle Blende" — Phase 14 Schritt 8 — Status: Fertig — echte monokulare Tiefenschätzung (MiDaS v2.1 small), Fokuspunkt per Klick, variabler Unschärferadius nach Tiefendifferenz
- [x] KI-Stiltransfer zwischen Fotos — Phase 14 Schritt 9 — Status: Fertig (eingeschränkt) — fünf real lizenzierte feste `fast_neural_style`-Stile statt eines beliebigen Referenzfotos (kein lizenzklares Modell dafür gefunden, siehe ADR-0041 Nachtrag IX)
- [x] Himmelsaustausch mit automatischer Neubelichtung — Phase 14 Schritt 10 — Status: Fertig (minimaler Umfang) — klassische Segmentierungs-Heuristik + Nutzerfoto als neuer Himmel, grobe Farbangleichung des Vordergrunds; ohne Deckkraft-Regler und mit reduzierter Testabdeckung

## Technische Grundlage (Phase 1, keine Endnutzer-Features)

- [x] Rust-Workspace mit Crate-Grenzen (`apx-core`, `apx-raw`, `apx-catalog`, `apx-app`) — Phase 1 — Status: Fertig
- [x] `apx-core` (IDs, AppError, AppPaths, Settings, Logging) — Phase 1 — Status: Fertig
- [x] SQLite-Katalog mit versionierten Migrationen — Phase 1 — Status: Fertig
- [x] RAW-Dekodierung (provisorische Kette, Formate CR2/CR3/NEF/ARW/RAF/ORF/RW2/DNG + JPEG/PNG/TIFF) — Phase 1 — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0007 — Golden-Image-Tests gegen echte Kameradateien fehlen noch, Netzwerkzugriff auf raw.pixls.us blockiert)
- [x] Custom-Protokoll-Handler für Bildübertragung — Phase 1 — Status: Fertig
- [x] Viewer mit Zoom/Pan (Canvas 2D, provisorisch) — Phase 1 — Status: Fertig
- [x] Testabdeckung (Rust-Unit-/Integrationstests, Vitest, Playwright-E2E) — Phase 1 — Status: Fertig (abweichend, siehe DECISIONS.md ADR-0010 — Playwright läuft gegen den Produktions-Build im Browser mit simulierter Tauri-Brücke, nicht gegen die kompilierte native App; echtes natives E2E bräuchte `tauri-driver` + WebdriverIO)
- [x] CI (Windows/macOS/Linux, fmt/clippy/test/build) — Phase 1 — Status: Fertig (`.github/workflows/ci.yml`; volles `tauri build` mit Installer/Signierung als eigener `release`-Job seit Phase 10 Schritt 11, siehe oben)
