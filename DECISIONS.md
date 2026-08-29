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

**Status:** Angenommen, vom Nutzer ausdrücklich bestätigt ("Ja, LGPL-2.1
für rawler akzeptieren")
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

**Entscheidung (vom Nutzer bestätigt):** `rawler` verwenden, LGPL-2.1 als bewusste Ausnahme akzeptieren, und die Ausnahme durch technische Maßnahmen entschärfen:
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

---

## ADR-0009: `apx://`-URLs als opake, über `convertFileSrc` kodierte Pfade statt echtem Query-String

**Status:** Angenommen
**Kontext:** `PHASE1_PROMPT.md` Abschnitt 6 illustriert das Protokoll mit
`apx://preview/<photo_id>?level=0`. Tauris offizieller, plattformneutraler
Weg, eine Custom-Protocol-URL im Frontend zu bauen, ist
`convertFileSrc(pfad, "apx")` aus `@tauri-apps/api/core` — nötig, weil sich
das URL-Schema zwischen Plattformen unterscheidet (macOS/Linux:
`apx://localhost/<pfad>`; Windows/Android: `http://apx.localhost/<pfad>`,
weil WebView2 und Android WebView keine beliebigen Custom-Schemes für
Netzwerk-Requests erlauben). Ein Blick in Tauris `convertFileSrc`-Quelle
zeigt: die Funktion `encodeURIComponent`-kodiert den **gesamten** `pfad`-
String als **ein einziges** Pfadsegment. Ein `?level=0` im Eingabestring
würde also mitkodiert (`%3Flevel%3D0`) und käme auf Rust-Seite nicht als
echter Query-String an, sondern als Teil eines einzigen, weiterhin
prozentkodierten Pfadsegments — `http::Uri::query()` bliebe `None`.
**Entscheidung:** Statt eines echten Query-Strings kodiert das Frontend
Anfragen als `convertFileSrc("preview/<photo_id>/<level>", "apx")` bzw.
`convertFileSrc("image/<photo_id>/<max_edge_oder_'full'>", "apx")` — die
gesamte Information steckt in einem einzigen, von `convertFileSrc` korrekt
prozentkodierten Pfadsegment. Der Rust-Handler dekodiert dieses Segment
selbst (`percent-encoding`-Crate) und splittet es an `/`. Das funktioniert
identisch auf allen drei Plattformen, ohne dass das Frontend das
Betriebssystem selbst erkennen müsste.
**Konsequenzen:** Die tatsächliche Pfadstruktur weicht in der Notation
leicht vom illustrativen Beispiel in `PHASE1_PROMPT.md` ab (Segmente statt
Query-Parameter) — funktional identisch (dieselben Informationen, dieselbe
Semantik: Foto-ID, gewünschte Auflösungsstufe/Kantenlänge). Diese
Abweichung ist hier dokumentiert, wie in `SPEC.md` Abschnitt 6 gefordert.

---

## ADR-0010: Playwright testet das Frontend gegen einen simulierten Tauri-Bridge, nicht die kompilierte native App

**Status:** Angenommen, mit klar benanntem Rest-Risiko
**Kontext:** `SPEC.md`s Tech-Stack-Tabelle nennt Playwright für E2E-Tests,
und `PHASE1_PROMPT.md` Abschnitt 8 verlangt ein E2E-Szenario, das die
komplette App abdeckt: „App starten → Ordner importieren → Thumbnails
erscheinen → Bild anklicken → Viewer zeigt es → Zoom 1:1 → Neustart →
Katalog ist noch da." Das ist ein Test der **kompilierten nativen
Tauri-App**, nicht nur der Web-Inhalte.

Playwright steuert Browser über das Chrome DevTools Protocol (CDP) an.
Tauris WebView ist plattformabhängig: WebView2 unter Windows basiert auf
Chromium und unterstützt CDP (`--remote-debugging-port`), WKWebView unter
macOS und WebKitGTK unter Linux tun das **nicht**. Ein CDP-basierter
Playwright-Test gegen die echte App liefe also nur auf Windows —
inakzeptabel für eine Anforderung, die alle drei Plattformen abdecken
muss. Der von Tauri selbst empfohlene, plattformunabhängige Weg für
echte native E2E-Tests ist `tauri-driver` (WebDriver-Protokoll, nutzt je
Plattform den nativen WebView-Treiber) zusammen mit WebdriverIO oder
Selenium — nicht Playwright.

**Entscheidung:** Für Phase 1 wird Playwright wie im Tech-Stack
vorgesehen eingesetzt, aber gegen den **Vite-Dev-/Preview-Server mit
einem simulierten `window.__TAURI_INTERNALS__`** (`invoke`/`convertFileSrc`
geben kontrollierte Testdaten zurück, siehe `frontend/e2e/`) statt gegen
die kompilierte App. Das deckt zuverlässig und plattformunabhängig ab:
Layout, Dark-Theme, Zustand-Store-Verhalten, Tastenkürzel, Filmstreifen-
Virtualisierung, Viewer-Interaktion — alles, was sich rein im Frontend
abspielt. Die **Backend-Anteile** des E2E-Szenarios (echter Import,
echte Dekodierung, echte Katalog-Persistenz über einen Neustart hinweg)
sind stattdessen durch Rust-Integrationstests abgedeckt, die exakt
dieselbe Logik über dieselben Schnittstellen ausführen wie die App selbst
(`apx-app`s `import_run_handles_three_valid_and_one_broken_file`,
`apx-catalog`s `open_on_disk_persists_across_reopen`) — nur eben nicht
durch Klicks in einem echten Fenster ausgelöst.
**Konsequenzen:** Es fehlt ein Test, der wirklich **Klick für Klick durch
die kompilierte App** geht und dabei Frontend und Backend gemeinsam über
die tatsächliche Tauri-IPC-Bridge prüft (z. B. ein Regressionsfehler
ausschließlich in der Verdrahtung zwischen echtem Command und echtem
Frontend-Aufruf, der weder im Rust-Test noch im simulierten
Playwright-Test auffiele). Das ist eine bewusste, dokumentierte Lücke
für Phase 1, keine stillschweigend schwächere Umsetzung. Empfehlung für
eine spätere Phase: `tauri-driver` + WebdriverIO als zusätzliche,
eigene Test-Stufe ergänzen, sobald der Funktionsumfang das rechtfertigt.

---

## ADR-0011: Phase 2 umfasst genau sieben Grundeinstellungs-Regler, nicht zwölf

**Status:** Angenommen
**Kontext:** `SPEC.md` §5s Phasenplan-Satz für Phase 2 nennt namentlich
sieben Regler: „die Grundeinstellungs-Regler (WB, Belichtung, Kontrast,
Lichter, Tiefen, Weiß, Schwarz)". `FEATURES.md` hatte davon abweichend
alle zwölf „Grundeinstellungen"-Regler aus `SPEC.md` §3.2 (zusätzlich
Textur, Klarheit, Dunst entfernen, Dynamik, Sättigung) als Phase 2
markiert — eine Diskrepanz zwischen der maßgeblichen Phasenplan-Zeile
und `FEATURES.md`s eigener, weiter gefasster Interpretation.

**Entscheidung:** `SPEC.md` §5 ist die maßgebliche Quelle für den
Phasen-Umfang. Phase 2 umfasst genau die sieben genannten Regler.
Textur/Klarheit/Dunst entfernen/Dynamik/Sättigung wandern in
`FEATURES.md` zu Phase 4, wo sie fachlich ohnehin neben Gradationskurve/
HSL/Farbmischer/Details stehen — Werkzeuge, die eher „gestalterische
Nachbearbeitung" als „grundlegende Tonwertkorrektur" sind.

**Konsequenzen:** Phase 2 bleibt beim in `SPEC.md` beschriebenen,
kleineren Umfang. `FEATURES.md`s eigene Präambel erlaubt genau diese Art
von Verfeinerung ausdrücklich ("wird beim Start der jeweiligen Phase in
PLAN.md verfeinert... Änderungen gehören dann in DECISIONS.md"). Diese
Entscheidung wurde stellvertretend getroffen, ohne Rückfrage beim
Nutzer, da sie den Umfang verkleinert (nicht wesentlich vergrößert) und
rein fachlich-technischer Natur ist.

---

## ADR-0012: Ein Crate `apx-pipeline`, nicht zwei (`apx-pipeline` + `apx-gpu`)

**Status:** Angenommen
**Kontext:** `ARCHITECTURE.md`s bisheriger Phase-2-Platzhalter nannte
zwei mögliche Crate-Namen nebeneinander, ohne zu entscheiden, ob es ein
oder zwei Crates werden.

**Entscheidung:** Ein einziges neues Crate, `apx-pipeline`, enthält das
EDL-Datenmodell, die wgpu-Anbindung, den Tile-Cache und das
Farbmanagement (`lcms2`). Begründung: Phase 1 hält den Workspace bewusst
bei vier Crates mit klaren Grenzen; eine Aufspaltung „GPU-Kontext" vs.
„EDL/Pipeline-Logik" in zwei Crates würde nur künstliche Grenzen
zwischen eng zusammenhängendem Code ziehen (der EDL-Interpreter *ist*
der wgpu-Aufrufer) ohne einen erkennbaren Vorteil für Testbarkeit oder
Wiederverwendbarkeit — anders als z. B. die `apx-raw`/`apx-catalog`-
Trennung, die zwei fachlich unabhängige Verantwortlichkeiten trennt.

**Konsequenzen:** `apx-pipeline` hängt von `apx-core` und `apx-raw` ab,
nicht von `apx-catalog` (siehe ADR-0013). `apx-app` hängt zusätzlich von
`apx-pipeline` ab, wie von den anderen drei Crates auch.

---

## ADR-0013: EDL wird als JSON in einem `apx-core`-Umschlagtyp gespeichert, nicht als CBOR, nicht direkt von `apx-catalog` interpretiert

**Status:** Angenommen
**Kontext:** `SPEC.md` §2.1 lässt das Serialisierungsformat offen
("als JSON/CBOR"). Zusätzlich stellt sich die Frage, wo der konkrete
EDL-Rust-Typ leben soll, ohne die Abhängigkeitsrichtung
`apx-catalog` ↛ `apx-pipeline` zu verletzen (siehe ADR-0012).

**Entscheidung:** JSON, nicht CBOR — menschenlesbar/diffbar (wichtig für
Debugging und die später geplante „Verlaufs-Vergleich"-Funktion),
`serde_json` ist bereits Workspace-Abhängigkeit, und die Nutzlast ist für
Phase 2 klein (7 Zahlen), sodass CBORs Kompaktheitsvorteil nicht ins
Gewicht fällt. `apx-core` bekommt einen minimalen, versionsmarkierten
Umschlagtyp `EdlEnvelope { schema_version: u32, payload:
serde_json::Value }`. `apx-catalog` speichert nur diesen Umschlag (als
`TEXT`-Spalte) und muss `payload` nie verstehen — nur `apx-pipeline`
kennt die konkrete `EdlV1`-Struct und entpackt `payload` hinein.

**Konsequenzen:** `apx-catalog` bleibt unabhängig von `apx-pipeline`.
Sollten spätere Phasen große Kurven-/Masken-Daten ins EDL bringen und
JSON-Größe/Parse-Geschwindigkeit zum echten Problem werden, ist ein
Wechsel zu CBOR eine lokale Änderung in `apx-pipeline` (Serialisierung)
plus einer neuen Schema-Version — kein Architekturbruch.

---

## ADR-0014: Verlauf/Undo-Redo als eigene SQLite-Tabelle vollständiger EDL-Schnappschüsse

**Status:** Angenommen
**Kontext:** `SPEC.md` §3.4/§5 verlangt „Verlauf mit unbegrenzten,
benennbaren, klickbaren Schritten (Undo/Redo)". Zwei Modellierungen
kämen infrage: ein Operations-Log (jede Regler-Änderung als ein Eintrag,
EDL wird durch Abspielen aller Einträge rekonstruiert) oder eine Tabelle
vollständiger EDL-Schnappschüsse pro Schritt.

**Entscheidung:** Eigene Tabelle `edit_history` mit vollständigen
EDL-Schnappschüssen pro Zeile (`photo_id`, `sequence`, `label`,
`edl_json`, `created_at`), plus ein 1-Zeile-pro-Foto-Zeiger
`edit_current`. Begründung: „Springe zu einem beliebigen früheren
Schritt", „benenne einen Schritt" und „vergleiche zwei beliebige
Schritte" (spätere Phasen) sind mit vollständigen Schnappschüssen
triviale SQL-Abfragen, während ein Operations-Log für dieselben
Anfragen erst wieder abgespielt werden müsste. Für Phase 2 gilt zudem:
Wiederholen (Redo) nach einer neuen Bearbeitung wird **nicht**
aufbewahrt (keine Verzweigung) — neue Bearbeitung nach einem Rückgängig
verwirft die „Zukunft", wie in den meisten Bildbearbeitungsprogrammen
üblich. Verzweigende Historie ist von `SPEC.md` nicht für Phase 2
gefordert.

**Konsequenzen:** Migration `migrations/0002_edits.sql` ist rein additiv
(ändert `photos` nicht), `edit_current` ist eine eigene Tabelle statt
einer neuen Spalte auf `photos`, um die Phase-1-Tabelle unangetastet zu
lassen.

---

## ADR-0015: `apx-raw` bekommt einen additiven `decode_linear()`-Einstiegspunkt statt den bestehenden Vertrag zu ändern

**Status:** Angenommen
**Kontext:** Der heutige `apx_raw::decode()`/`DecodedImage`-Pfad
bäckt einen festen (nicht einstellbaren) Weißabgleich und eine feste
sRGB-Gammakurve fest ein — genau das, was Phase 2 einstellbar machen
muss (Weißabgleich-Regler, Belichtung/Ton). Eine Änderung an `decode()`
selbst würde die Verträge der bestehenden Phase-1-Aufrufer (Vorschau-
Erzeugung im Import-Job, Vorschau-/Vollbild-Routen im Protokoll-Handler)
brechen.

**Entscheidung:** Neuer, rein additiver Einstiegspunkt
`apx_raw::decode_linear()`, der die Kette nach CFA-Laden/Demosaicing/
Schwarz-Weiß-Normalisierung stoppt — vor dem bisherigen festen
Weißabgleich-Multiplikator und vor der Gammakurve. `apx-pipeline`
übernimmt ab diesem linearen Zwischenergebnis. `decode()`/`DecodedImage`
bleiben für alle bestehenden Phase-1-Aufrufer unverändert.

**Konsequenzen:** Es gibt vorübergehend zwei Ausstiegspunkte aus der
RAW-Dekodierkette in `apx-raw` (der alte, feste `decode()`-Pfad für
Vorschauen/Thumbnails; der neue `decode_linear()`-Pfad für die
Entwickeln-Ansicht) — bewusst in Kauf genommen, statt Phase 1s
funktionierenden Code für eine Anforderung umzubauen, die er nicht
erfüllen muss (Thumbnails brauchen keinen einstellbaren Weißabgleich).

---

## ADR-0016: Die interaktive Entwickeln-Route liefert rohe RGBA8-Bytes statt PNG/JPEG

**Status:** Angenommen
**Kontext:** Die bestehenden `apx://preview/…`- und `apx://image/…`-
Routen liefern JPEG bzw. PNG. `SPEC.md` §2.4 verlangt für die neue
interaktive Entwickeln-Ansicht „Regler-Bewegung → sichtbares Update:
< 16 ms bei 24-MP-Proxy" — eine erneute PNG-Kodierung (DEFLATE-
Kompression) bei jedem Regler-Tick würde spürbar Zeit aus diesem engen
Budget verbrauchen, nur um die Bytes im Frontend gleich wieder zu
dekodieren, bevor sie als WebGL2-Textur hochgeladen werden.

**Entscheidung:** Die neue `apx://develop/<id>/<edl_hash>/<max_edge>`-
Route liefert rohe, unkomprimierte RGBA8-Bytes (`Content-Type:
application/octet-stream`, mit Breite/Höhe in einem kleinen Header)
statt PNG. Die bestehenden `preview`-/`image`-Routen bleiben unverändert
bei JPEG/PNG — diese Abweichung gilt ausschließlich für die neue,
performance-kritische Route.

**Konsequenzen:** Größere Rohdatenmenge pro Anfrage als bei komprimierten
Formaten — bei lokaler IPC über `localhost`/den Tauri-Protokoll-Handler
ist das akzeptabel (kein Netzwerk, keine Bandbreitenbegrenzung), der
Kompressions-Zeitgewinn wiegt hier schwerer als die Byte-Ersparnis.

---

## ADR-0017: Fusionierter Compute-Shader für den interaktiven Pfad, separate Shader pro Regler für Spec-Konformität und Tests

**Status:** Angenommen
**Kontext:** `SPEC.md` §6 verlangt „jedes Modul mit eigenem Shader,
eigenem Test". Ein einzelner GPU-Dispatch pro Regler-Änderung würde bei
sieben Reglern potenziell sieben aufeinanderfolgende Dispatch+Rücklese-
Zyklen bedeuten — jeder mit eigenem Overhead, der zusammen das 16-ms-
Budget gefährdet.

**Entscheidung:** Jeder der fünf Regler-Module (Weißabgleich, Belichtung,
Kontrast, Lichter+Tiefen, Weiß+Schwarz) bekommt trotzdem seinen eigenen
WGSL-Shader und eigene Tests (erfüllt SPEC.md wörtlich). Zusätzlich gibt
es einen fusionierten Shader (`basic_fused.rs`), der dieselbe Mathematik
aller fünf Module in einem einzigen GPU-Aufruf kombiniert. Der
interaktive Vorschau-Pfad (bei jedem Regler-Tick) nutzt ausschließlich
den fusionierten Shader — ein Dispatch statt fünf. Ein Abgleichstest
stellt sicher, dass fusionierter und einzelne Shader auf identischem
Input dasselbe Ergebnis liefern (innerhalb der üblichen Gleitkomma-
Toleranz).

**Konsequenzen:** Etwas Code-Duplikation zwischen Einzel- und fusioniertem
Shader (dieselbe Mathematik zweimal ausgedrückt) — bewusst in Kauf
genommen zugunsten von Performance UND Spec-Konformität statt eines
Kompromisses, der keines von beidem vollständig erfüllt.

---

## ADR-0018: `zundo` für Undo/Redo im Frontend

**Status:** Angenommen
**Kontext:** Der Zustand-Store nutzt bereits `immer`; `FEATURES.md`
verlangt für Phase 2 „Verlauf mit unbegrenzten, benennbaren, klickbaren
Schritten (Undo/Redo)" im Frontend (zusätzlich zur dauerhaften
`edit_history`-Tabelle aus ADR-0014, die den App-Neustart überlebt — die
Frontend-Historie ist die *aktive Sitzung*, nicht dasselbe System).

**Entscheidung:** `zundo`, eine kleine, MIT-lizenzierte Bibliothek, die
gezielt bestehende Zustand-Stores (auch mit `immer`) um Verlauf
erweitert und Einträge bündelt/entprellt, statt bei jeder Mausbewegung
einen Schritt anzulegen. Eine selbstgebaute Lösung müsste dieselbe
Entprellungs-Logik ohne erkennbaren Vorteil neu erfinden.

**Konsequenzen:** Eine weitere kleine Frontend-Abhängigkeit, in
`THIRD_PARTY.md` einzutragen. Die `zundo`-Historie lebt nur im
Arbeitsspeicher der laufenden Sitzung; das Überleben eines Neustarts
läuft ausschließlich über `edit_history` (ADR-0014).
