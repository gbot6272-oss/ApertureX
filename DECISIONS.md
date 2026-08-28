# DECISIONS.md — Architecture Decision Records

Ein Eintrag pro Architekturentscheidung, im ADR-Format. Einträge werden nicht rückwirkend verändert — eine überholte Entscheidung bekommt einen neuen Eintrag mit Verweis auf den alten (`Status: Abgelöst durch ADR-00xx`).

---

## ADR-0001: SQLite-Zugriff über `rusqlite` statt `sqlx`

**Status:** Angenommen
**Kontext:** `SPEC.md` schreibt `sqlx` vor, erlaubt Abweichung nur mit Begründung. `sqlx`s Compile-Zeit-Prüfung von SQL-Statements braucht entweder eine erreichbare Datenbank beim Bauen oder eine gepflegte `.sqlx`-Offline-Cache-Datei. In dieser Phase ändert sich das Schema noch häufig (Migration 1 ist erst der Anfang), das würde ständig Reibung erzeugen — jede Schemaänderung bräuchte einen `cargo sqlx prepare`-Schritt, sonst bricht CI.
**Entscheidung:** `rusqlite` mit `bundled`-Feature (SQLite wird mitkompiliert, keine System-Abhängigkeit). Repository-Funktionen sind typisiert und gekapselt in `apx-catalog`, SQL bleibt vollständig dort.
**Konsequenzen:** Kein Compile-Zeit-Typecheck von SQL wie bei `sqlx` — dafür sorgen Repository-Tests (Round-Trip pro Tabelle) für Absicherung. `rusqlite` ist synchron; Aufrufe aus `apx-app` laufen über `spawn_blocking`, damit der Tauri-Async-Kontext nicht blockiert.
**Lizenz:** `rusqlite` MIT, gebündeltes SQLite Public Domain. Unkritisch.

---

## ADR-0002: RAW-Dekodierung über `rawler` — LGPL-2.1-Hinweis

**Status:** Angenommen, **explizit lizenzrechtlich zu bestätigen**
**Kontext:** `SPEC.md` Abschnitt 6 verlangt: „Nichts mit GPL im Kern, außer du weist mich ausdrücklich darauf hin." Der Phase-1-Prompt schreibt `rawler` als Bibliothek vor und verbietet eigenes RAW-Parsing.

Ich habe die Lizenzlage der realistischen Alternativen geprüft:

| Crate | Lizenz |
|---|---|
| `rawler` (dnglab-Projekt) | LGPL-2.1 |
| `rawloader` (Vorgänger von rawler, unmaintained) | LGPL-2.1 |
| `quickraw` | LGPL-2.1 |
| `libraw`-Bindings (C-FFI) | Binding selbst MIT, verlinkt aber `libraw` (LGPL-2.1 **oder** CDDL-1.0 **oder** LibRaw Software License) |

**Es gibt aktuell keine dokumentierte, breit RAW-Formate abdeckende Rust-Bibliothek mit rein permissiver Lizenz (MIT/Apache/BSD).** Das liegt an der Herkunft dieses Ökosystems aus `dcraw`, dessen Abkömmlinge durchgängig LGPL sind. Ein eigener RAW-Parser wäre die einzige GPL-freie Alternative, widerspricht aber ausdrücklich der Vorgabe „Baue kein eigenes RAW-Parsing" im Phase-1-Prompt und wäre für Phase 1 nicht seriös leistbar (RAW-Formate sind je Hersteller eigene, teils unter NDA stehende Binärformate).

**Damit hiermit ausdrücklich darauf hingewiesen** (wie von `SPEC.md` gefordert), bevor Code geschrieben wird.

**Entscheidung (vorgeschlagen, wartet auf Bestätigung):** `rawler` verwenden, LGPL-2.1 als bewusste Ausnahme akzeptieren, und die Ausnahme durch technische Maßnahmen entschärfen:
1. `apx-raw` bleibt ein eigener, klar abgegrenzter Crate (bereits durch Workspace-Struktur erzwungen) — die LGPL-Komponente ist isoliert, nicht mit Geschäftslogik vermischt.
2. Sobald Distribution ansteht (ab Phase 10 / Installer), `apx-raw` als dynamisch nachladbare Komponente bauen (`cdylib` oder Plugin-Grenze), damit Nutzer sie gemäß LGPL §6 durch eine modifizierte Version ersetzen können, statt sich auf eine für Rust unklare statische Verlinkung zu verlassen. Diese Maßnahme wird in Phase 10 konkret umgesetzt und hier nur vorgemerkt.
3. Vollständiger Lizenztext und Quellverweis in `THIRD_PARTY.md`.
**Konsequenzen:** Falls das Gesamtprodukt closed-source vertrieben werden soll, ist Punkt 2 nicht optional, sondern Voraussetzung für Rechtssicherheit. Falls Aperture X ohnehin quelloffen wird, entfällt die Dringlichkeit weitgehend. **Diese Weichenstellung (closed-source vs. quelloffen) betrifft den Gesamtumfang des Projekts und wird hier nicht einseitig entschieden.**

---

## ADR-0003: Bildübertragung Backend→Frontend über Custom-Protokoll-Handler

**Status:** Angenommen
**Kontext:** Bilddaten (Vorschauen wie Vollbilder) müssen häufig und schnell vom Rust-Backend ins WebView. Tauri-Commands mit Base64-kodierten Bildern im Rückgabewert wurden erwogen.
**Entscheidung:** Custom-Protokoll-Handler (`apx://preview/<id>?level=`, `apx://image/<id>?max_edge=`), der direkt Binärdaten mit korrektem `Content-Type`/`Cache-Control` liefert. Das Frontend nutzt die URL direkt in `<img>` bzw. per `fetch`+`createImageBitmap`.
**Konsequenzen:** Kein Base64-Overhead (+33 % Größe, zusätzliche Kopie im JS-Heap), Browser-eigenes Caching greift, parallele Anfragen werden im Handler dedupliziert. Mehraufwand: Protokoll-Handler-Routing und Abbruchlogik (laufende Dekodierung bei Bildwechsel canceln) müssen selbst gebaut werden.

---

## ADR-0004: Viewer in Phase 1 auf Canvas 2D statt WebGL/WebGPU

**Status:** Angenommen, befristet auf Phase 1
**Kontext:** `SPEC.md` verlangt für den fertigen Viewer WebGL2/WebGPU ohne DOM-Rendering im Bildbereich. Phase 1 hat aber explizit noch keine GPU-Pipeline (die kommt in Phase 2 mit `wgpu`).
**Entscheidung:** Für Phase 1 reicht ein `<canvas>` mit 2D-Kontext und `ImageBitmap` als Quelle — es wird nur ein bereits fertig dekodiertes Bild dargestellt, Zoom/Pan sind reine Transformationen, keine Pixel-Verarbeitung. Das hält Phase 1 schlank und vermeidet, eine WebGL-Infrastruktur zu bauen, die in Phase 2 ohnehin durch die wgpu-Pipeline ersetzt bzw. eng verzahnt wird.
**Konsequenzen:** Der Viewer-Code wird in Phase 2 nicht 1:1 weiterverwendet, sondern um die GPU-Pipeline-Anbindung erweitert/ersetzt — das ist im Phase-1-Prompt so vorgesehen und im Code entsprechend kommentiert. Kein Zwischenschritt „WebGL ohne Pipeline" wird gebaut, das wäre Doppelarbeit.

---

## ADR-0005: IDs als UUIDv7-Newtypes

**Status:** Angenommen
**Kontext:** Katalogeinträge (Fotos, Ordner …) brauchen stabile, eindeutige IDs, die sich gut in SQLite (`TEXT PRIMARY KEY`) und in Cache-Pfaden (`previews/<xx>/<id>_<level>.jpg`) verwenden lassen.
**Entscheidung:** `PhotoId`, `FolderId`, `CatalogId` als typisierte Newtypes über UUIDv7. UUIDv7 ist zeitlich sortierbar (Zeitstempel in den High-Bits), das hilft bei Index-Lokalität und macht „neueste zuerst"-Abfragen ohne extra Sortierspalte günstiger.
**Konsequenzen:** Erfordert ein UUID-Crate mit v7-Unterstützung (`uuid` mit `v7`-Feature). Newtype-Wrapper verhindern, dass eine `PhotoId` versehentlich als `FolderId` verwendet wird (Typsicherheit statt „stringly typed" IDs).

---

## ADR-0006: Fehlerbehandlung mit `thiserror`, keine `unwrap()` außerhalb von Tests

**Status:** Angenommen
**Kontext:** `SPEC.md` Abschnitt 6 verlangt explizite Fehlerbehandlung überall.
**Entscheidung:** Ein gemeinsamer `AppError`-Typ in `apx-core` (`thiserror`), alle Crates geben `Result<T, AppError>` zurück. `clippy::unwrap_used` wird als Lint aktiviert (`-D clippy::unwrap_used` außerhalb von `#[cfg(test)]`), damit das in CI erzwungen statt nur vereinbart ist.
**Konsequenzen:** Etwas mehr Schreibaufwand an Fehlerstellen, dafür keine stillen Panics in Produktionscode.

---

## ADR-0007: Echte RAW-Testdateien fehlen — Netzwerkzugriff auf raw.pixls.us in dieser Sandbox blockiert

**Status:** Offen — Entscheidung/Freigabe durch Nutzer ausstehend
**Kontext:** `PHASE1_PROMPT.md` Abschnitt 8 verlangt für `apx-raw` "pro Format eine Datei in `testdata/`" plus Golden-Image-Tests gegen diese echten Kameradateien. Die empfohlene Quelle ist raw.pixls.us (CC0-lizenzierte RAW-Samples). Ein Zugriffsversuch (per `curl` **und** per `WebFetch`) auf `https://raw.pixls.us/` schlägt in dieser Sandbox-Session mit `EGRESS_BLOCKED` bzw. HTTP 403 fehl — die Netzwerk-Egress-Policy dieser Umgebung lässt den Host nicht zu. Das ist keine Design-Entscheidung, sondern eine Umgebungsgrenze dieser konkreten Sitzung.
**Was stattdessen existiert:** Alle Decoding-Algorithmen (Demosaicing bilinear + generischer Fallback, Farbmatrix-Herleitung, Gammakurve, Crop, Downsampling, Orientierung, EXIF-Zeitzonen-Parsing) sind über synthetische Unit-Tests mit selbst konstruierten Eingabedaten abgedeckt (39 Tests, alle grün) — das prüft die Korrektheit der Implementierung, aber nicht das Verhalten gegen reale Dateien der acht Zielformate.
**Optionen für den Nutzer:**
1. Netzwerk-Egress für `raw.pixls.us` (oder eine alternative CC0-Quelle) für diese Umgebung freischalten, dann hole ich die Dateien in dieser Session nach.
2. Der Nutzer lädt passende CC0-RAW-Samples selbst herunter und legt sie im Repo unter `testdata/` ab (mit Lizenzangabe für `THIRD_PARTY.md`) — ich ergänze dann die Golden-Image-Tests.
3. GitHub Actions selbst (die echten Runner haben regulären Internetzugang, anders als diese Sandbox) lädt die Testdateien bei Bedarf in einem eigenen CI-Schritt herunter, statt sie im Repo zu versionieren — spart Repo-Größe, macht CI aber von der Erreichbarkeit der externen Quelle abhängig.
**Entscheidung:** Noch nicht getroffen. Bis zur Klärung bleibt dieser Teilpunkt in `PLAN.md` als offen/blockiert markiert; der Rest von Phase 1 wird nicht darauf verzögert.

---

## ADR-0008: Katalog-Verbindung als einzelne `Connection` hinter einem `Mutex`

**Status:** Angenommen
**Kontext:** `PHASE1_PROMPT.md` Abschnitt 10 nennt als bekannten Fallstrick: "SQLite-Schreibzugriffe parallel → `database is locked`. Ein einziger Writer, Leser über einen Pool." Ein echter Connection-Pool mit getrennten Lese-Verbindungen (z. B. über `r2d2`) wäre möglich, bringt aber zusätzliche Abhängigkeiten und Komplexität (Pool-Konfiguration, Verbindungs-Recycling), die für Phase 1 (ein Nutzer, ein Prozess) keinen messbaren Vorteil bringt.
**Entscheidung:** `Catalog` hält eine einzelne `rusqlite::Connection` in einem `std::sync::Mutex`. Alle Zugriffe — lesend wie schreibend — werden dadurch auf Rust-Ebene serialisiert. Das erfüllt die Anforderung "ein einziger Writer" direkt und verhindert `database is locked`-Fehler aus konkurrierenden Zugriffen innerhalb des Prozesses vollständig, nicht nur im statistischen Regelfall. WAL-Modus bleibt aktiviert, damit externe Prozesse (z. B. ein DB-Browser zur Fehlersuche) weiterhin parallel lesend zugreifen können.
**Konsequenzen:** Innerhalb des Prozesses gibt es keine echte Nebenläufigkeit bei Katalogzugriffen — ein langer Schreibvorgang (z. B. ein Massenimport) blockiert währenddessen andere Katalog-Lesezugriffe. Für Phase 1 unproblematisch (Importe laufen ohnehin sequenziell pro Datei mit kurzen Einzeltransaktionen). Sollte Profiling in einer späteren Phase echte Kontention zeigen (z. B. Bibliotheks-Raster-Scrollen blockiert durch einen laufenden Import), wird das hier nachgerüstet — dokumentiert als möglicher Folge-ADR, nicht vorab spekulativ gebaut.
