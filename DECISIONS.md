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

**Entscheidung:** Die neue `apx://develop/<id>/<max_edge_oder_'full'>/<edl_json>`-
Route liefert rohe, unkomprimierte RGBA8-Bytes (`Content-Type:
application/x-apx-develop-rgba8`, mit Breite/Höhe als `u32`
little-endian in einem 8-Byte-Header vor den Pixeldaten) statt PNG. Die
bestehenden `preview`-/`image`-Routen bleiben unverändert bei JPEG/PNG —
diese Abweichung gilt ausschließlich für die neue, performance-kritische
Route.

**Umsetzungs-Korrektur gegenüber der ursprünglichen Plan-Notiz (Schritt
5):** Statt eines separaten `edl_hash`-Segments trägt die Route die
**vollständige JSON-Serialisierung des `EdlEnvelope`** direkt im
Pfad (`edl_json`) — ein reiner Hash hätte bedeutet, dass die Route den
zugehörigen EDL-Wert irgendwoher nachschlagen müsste, was während des
*Ziehens* eines Reglers (noch nicht committet, siehe ADR-0014) gar nicht
möglich ist. Derselbe String dient nebenbei unverändert als
Unterscheidungsmerkmal für den bestehenden Single-Flight-`ImageCache`
(zwei EDL-Zustände desselben Fotos erzeugen zwei Cache-Schlüssel) — ein
separater Hash-Mechanismus ist damit unnötig. `EdlV1`s Felder sind
ausschließlich Zahlen und enthalten daher nie ein `/`-Zeichen, das die
bestehende "erst dekodieren, dann an `/` aufteilen"-Pfadlogik stören
könnte.

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

## ADR-0018: Undo/Redo im Frontend direkt über `edit_history`, kein `zundo` (revidiert in Schritt 6)

**Status:** Angenommen (ursprüngliche Fassung revidiert bei der tatsächlichen
Implementierung in Phase 2 Schritt 6 — siehe „Revisions-Begründung" unten)
**Kontext:** Der Zustand-Store nutzt bereits `immer`; `FEATURES.md`
verlangt für Phase 2 „Verlauf mit unbegrenzten, benennbaren, klickbaren
Schritten (Undo/Redo)" im Frontend, zusätzlich zur dauerhaften
`edit_history`-Tabelle aus ADR-0014, die den App-Neustart überlebt.

**Ursprüngliche Entscheidung (Schritt 0):** `zundo`, eine kleine
Bibliothek, die bestehende Zustand-Stores um eine eigene, zusätzliche
Verlaufs-Ebene erweitert — mit der Begründung, sie bündle/entprelle
Einträge, statt bei jeder Mausbewegung einen Schritt anzulegen.

**Revisions-Begründung (Schritt 6):** Bei der tatsächlichen Umsetzung
stellte sich heraus, dass die Entprellung viel einfacher auf
UI-Ereignis-Ebene lösbar ist, ganz ohne State-Management-Bibliothek:
`DevelopSlider` ruft `onChange` (Live-Vorschau, kein Commit) bei jedem
Zwischenwert, aber `onCommit` (schreibt über `apply_develop_edit` in
`edit_history`) erst bei Loslassen/Blur/Doppelklick-Reset. Eine
zusätzliche, separate Frontend-Verlaufs-Ebene (`zundo`) hätte dann *zwei*
lose synchronisierte Historien bedeutet (die lokale und die in
`edit_history`) — mit dem Risiko, dass sie nach einem Fehler oder einer
Race Condition auseinanderlaufen. Stattdessen rufen die Undo-/Redo-Knöpfe
und Strg/Cmd+Z direkt `undo_develop_edit`/`redo_develop_edit` auf (siehe
`crates/apx-app/src/commands.rs`, Phase 2 Schritt 5) — Tauris IPC ist
lokal (kein Netzwerk) schnell genug, um pro Klick einen Roundtrip zu
rechtfertigen, und es gibt dadurch nur *eine* Quelle der Wahrheit für den
Verlauf statt zwei.

**Konsequenzen:** Keine zusätzliche Frontend-Abhängigkeit nötig (`zundo`
wurde nie zu `package.json` hinzugefügt, `THIRD_PARTY.md` bleibt
unverändert). Ein Undo/Redo-Klick löst einen Tauri-Command-Aufruf aus
(unmerklich schnell bei lokalem IPC); eine visuelle, benennbare,
klickbare Liste *aller* Verlaufsschritte (statt nur Rückgängig/
Wiederholen um jeweils einen Schritt) ist mit dieser Entscheidung
weiterhin möglich (über `list_edit_history`, noch nicht implementiert),
aber bewusst über Schritt 6s Mindestumfang hinaus zurückgestellt — der
kritische Pfad („interaktives Entwickeln" überhaupt vorführbar) hatte
Vorrang.

---

## ADR-0019: Feste Kamera→sRGB-Matrix + Gammakurve im Entwickeln-Renderpfad, wiederverwendet statt dupliziert

**Status:** Angenommen
**Kontext:** `apx_raw::decode_linear()` (ADR-0015) liefert absichtlich
unbalanciertes, noch nicht farbtransformiertes Kamera-RGB — die
Weißabgleich-Gains sind nutzerseitig verstellbar (siehe
`WhiteBalanceAdjustment`) und gehören deshalb in `apx-pipeline`, nicht in
`apx-raw`. Die anschließende Kamera-RGB→sRGB-Transformation (eine feste
3×3-Matrix aus den Kamera-Kalibrierungsdaten, siehe
`apx-raw/src/pipeline/color.rs`) ist dagegen **nicht** nutzerseitig
verstellbar — sie ist trotzdem nötig, sonst zeigt der Entwickeln-Viewer
falsche Farben (Kamera-RGB-Primärvalenzen unterscheiden sich spürbar von
sRGB). Ohne sie wäre die neue `develop`-Route zwar lauffähig, aber
farblich falsch — kein tragbarer Zustand für ein Bildbearbeitungswerkzeug.

**Entscheidung:** `apx_raw::LinearImage` bekommt ein zusätzliches Feld
`cam_to_srgb: [[f32; 3]; 3]` (Einheitsmatrix für Fallback-Formate ohne
Kamerakalibrierung). `apx-pipeline::color` wendet diese feste Matrix plus
dieselbe sRGB-Gammakurve an, die `apx_raw::decode()` für Phase-1-
Vorschauen bereits nutzt (`apx_raw::srgb_gamma`, zusätzlich als Teil von
`apx-raw`s öffentlicher API re-exportiert, statt die Formel ein zweites
Mal abzuschreiben) — Reihenfolge im Renderpfad ist damit: As-shot-
Weißabgleich + Nutzer-Shift → die sieben Regler (auf Kamera-RGB) → feste
Kamera→sRGB-Matrix → Gammakurve → RGBA8-Quantisierung.

**Bewusste Vereinfachung:** Die sieben Regler (insbesondere Kontrast/
Lichter/Tiefen/Weiß/Schwarz) wirken damit auf Kamera-RGB-Werten, nicht
auf einem farbmanagement-korrekten Arbeitsraum nach der Matrix — anders
als z. B. Lightroom/Capture One, die Tonwert-Werkzeuge meist nach der
Farbmatrix anwenden. Das kann bei starken Kamera-Matrizen zu leicht
abweichenden Farbverschiebungen zwischen den Kanälen führen. Für Phase 2s
Ziel „interaktives Entwickeln" ist das akzeptabel (die Regler bewegen
sich intuitiv in die richtige Richtung); eine Neuordnung (Matrix vor den
Reglern) ist ein möglicher Ausbau für eine spätere Phase, sobald echtes
ICC-Farbmanagement (`lcms2`, ursprünglich für `apx-pipeline::color`
vorgesehen, siehe `PLAN.md` Phase 2 Schritt 4) tatsächlich eingebaut
wird.

**Konsequenzen:** `apx-raw`s `pipeline`-Modul exponiert
`cam_to_srgb_matrix` zusätzlich als `pub(crate)` (vorher rein privat in
`pipeline::color`), damit `decode_raw_linear` sie berechnen kann, ohne
eine vollständige `ColorPipeline` (die zusätzlich Weißabgleich/Gamma
anwenden würde) zu instanziieren. Kein neues externes Crate, keine
`lcms2`-Abhängigkeit in diesem Schritt.

---

## ADR-0020: Entwickeln-Modul als zuschaltbares Overlay im bestehenden Viewer, WebGL2 für beide Pixelquellen

**Status:** Angenommen
**Kontext:** `PLAN.md` Phase 2 Schritt 6 verlangt „Viewer.tsx wechselt von
2D-Canvas auf WebGL2". Offen war dabei, ob das Entwickeln-Rendering die
bestehende Vorschau/Vollbild-Anzeige (schnelle, gecachte JPEG/PNG-Pfade
aus Phase 1) vollständig ersetzt, oder als zusätzlicher, zuschaltbarer
Modus daneben existiert.

**Entscheidung:** Ein neuer „Entwickeln"-Knopf (`Header.tsx`) schaltet
`developPanelOpen` um; nur währenddessen fragt der Viewer zusätzlich die
`develop/...`-Route ab (`hooks/useDevelopRender`) und zeigt deren
Ergebnis anstelle des bisherigen Vollbilds — ohne offenes Panel verhält
sich der Viewer exakt wie in Phase 1. Der Canvas selbst nutzt jetzt immer
WebGL2 (`lib/webgl.ts`, `QuadRenderer`) statt Canvas-2D, für beide Fälle:
ein dekodiertes `ImageBitmap` (bestehender Pfad) und ein rohes RGBA8
(neue Entwickeln-Route) werden über denselben Texturmechanismus
hochgeladen — Canvas-2D hätte für RGBA8-Rohdaten einen Umweg über
`ImageData` gebraucht. Die bestehende Zoom-/Pan-Geometrie
(`lib/viewerMath.ts`) bleibt unverändert wiederverwendet, nur der
Zeichenaufruf selbst wechselt von `ctx.drawImage` auf einen texturierten
Quad-Shader.

**Konsequenzen:** Geringes Regressionsrisiko für Phase 1s bestehende
Funktionalität (Standardzustand = Panel geschlossen = unverändertes
Verhalten, durch die bestehenden Playwright-Tests in `viewer-flow.spec.ts`
weiterhin abgedeckt, die nach diesem Umbau unverändert grün bleiben). Der
WebGL2-Rendercode selbst (`lib/webgl.ts`) ist mangels WebGL-Unterstützung
in `jsdom` nicht per Vitest testbar — wie schon die vorherige
Canvas-2D-Zeichenlogik nur indirekt über Playwright abgesichert
(„Canvas ist sichtbar", jetzt zusätzlich über die Entwickeln-Flow-Tests
in `develop-flow.spec.ts`, die echte Zustandsänderungen statt nur
Sichtbarkeit prüfen).

---

## ADR-0021: wgpu-GPU-Ausführungstests in CI empirisch bestätigt (Schritt 8) — kein "weich" mehr auf Dauer

**Status:** Angenommen (empirisch verifiziert)
**Kontext:** ADR-0012 und `PLAN.md`s Risikoliste ließen offen, ob die
GitHub-Actions-Runner (Linux/macOS/Windows) tatsächlich einen wgpu-Adapter
finden — die `gpu_matches_cpu`-artigen Tests waren bewusst so gebaut, dass
ein fehlender Adapter zu einem klar begründeten, sichtbaren "übersprungen"
statt einem stillen Erfolg oder Hartabbruch führt. Ohne `--nocapture`
sah ein grüner CI-Lauf aber identisch aus, egal ob der Adapter gefunden
oder der Test nur übersprungen wurde — die Frage blieb unbeantwortet.

**Entscheidung/Befund:** `cargo test --workspace -- --nocapture` in
`ci.yml` aktiviert; im CI-Lauf für Commit `ca2e39f` (Run-ID 33236428223)
enthält **keines** der drei Rust-Job-Logs (`ubuntu-latest`,
`macos-latest`, `windows-latest`) die Zeichenkette "übersprungen: kein
GPU-Adapter" oder `GpuUnavailable` — alle sechs
`stages::*::tests::gpu_matches_cpu`-Tests liefen auf allen drei
Plattformen als echte GPU-Dispatches durch. Damit ist empirisch bestätigt
(nicht mehr nur angenommen): GitHub-Actions-`ubuntu-latest`-Runner bringen
von Haus aus einen nutzbaren Software-Vulkan-Adapter mit (kein
zusätzliches Mesa-`apt-get`-Paket war nötig); `macos-latest` und
`windows-latest` finden ebenfalls einen Adapter (vermutlich Metal bzw.
WARP/DX12 — welches Backend genau, ist ohne gesetztes `RUST_LOG` aus den
Logs nicht ablesbar, war für die Kernfrage aber nicht nötig).

**Konsequenzen:** GPU-Ausführungstests gelten ab sofort wieder als echte
Pflicht (nicht dauerhaft "weich" belassen, wie ADR-0012 es für den Fall
vorsah, dass ein Runner keinen Adapter hat) — ein zukünftiges
"übersprungen" auf einem dieser drei Runner wäre eine echte Regression
(z. B. ein Runner-Image-Wechsel), die untersucht werden müsste, nicht
länger stillschweigend akzeptiert. `ci.yml`s `-D clippy::unwrap_used`
und `apx-app`s fehlendes `#![deny(clippy::unwrap_used)]` wurden bei
diesem Durchgang zusätzlich als bestehende, unabhängige CI-Lücken
gefunden und geschlossen.

---

## ADR-0022: Phase 3 umfasst `SPEC.md` §5s Phase-3-Satz, nicht §3.1s vollen BIBLIOTHEK-Katalog

**Status:** Angenommen
**Kontext:** Wie schon bei Phase 2 (ADR-0011: 7 statt 12 Regler) hatte
`FEATURES.md` deutlich mehr Punkte auf Phase 3 getaggt, als `SPEC.md`
§5s Phasenplan-Satz eigentlich meint — §3.1 zählt den vollständigen
Lightroom-BIBLIOTHEK-Funktionsumfang auf (Gesichtserkennung, virtuelle
Kopien, Stapel, Sekundäres Display, Schlagwort-Synonyme/Auto-
Vervollständigung, Katalog-Backup/-Reparatur/-Zusammenführen,
vollständiger EXIF/IPTC/XMP-Editor mit Sidecar-Export, intelligente
Sammlungen mit verschachtelten Regeln, Perceptual-Hash-Duplikaterkennung,
DNG-Konvertierung), §5s Satz nennt für Phase 3 nur: "Import, Ordner,
Raster, Filmstreifen, Vorschau-Generierung, Bewertungen/Flaggen/Farben,
Sammlungen, Filter, Metadaten-Panel, FTS-Suche".

**Entscheidung:** Phase 3 = §5s Satz wörtlich genommen. Alle darüber
hinausgehenden Punkte sind in `FEATURES.md` auf die Phase umgetaggt, zu
der sie inhaltlich passen (meist Phase 6 „Sammlungen/Metadaten-Ausbau"
oder Phase 9 „Politur/fortgeschrittene Werkzeuge" — siehe die einzelnen
Zeilen in `FEATURES.md` §3.1 für die genaue Zuordnung). Zwei Lücken dabei
gefunden und ergänzt: `FEATURES.md` hatte weder eine Zeile für
"Volltextsuche (FTS5)" noch für ein einfaches "Metadaten-Panel" (nur den
späteren vollständigen EXIF/IPTC/XMP-Editor) — beide sind aber explizit
Teil von §5s Phase-3-Satz und wurden neu ergänzt.

**Konsequenzen:** Wie bei ADR-0011 eine reine Scope-Verkleinerung, keine
-Vergrößerung — der Nutzer muss dazu nicht befragt werden (siehe
`SPEC.md`s Präambel zu Kleinigkeiten, die den Umfang nicht wesentlich
verändern).

---

## ADR-0023: Bewertung/Flagge/Farbe als Spalten, Schlagworte flach, Sammlungen nur manuell, FTS5 als External-Content-Tabelle

**Status:** Angenommen
**Kontext:** Phase 3s neues DB-Schema (Migration `0003_library.sql`)
braucht Entscheidungen zu vier unabhängigen Datenmodell-Fragen.

**Entscheidungen:**
1. **Bewertung (`rating`), Flagge (`flag`), Farbmarkierung
   (`color_label`) sind direkte Spalten auf `photos`**, keine eigene
   Tabelle — konsistent mit dem bestehenden `missing`-Spalten-Muster aus
   Phase 1, da es sich um einfache Skalarwerte pro Foto handelt.
2. **Schlagworte sind in Phase 3 eine flache Liste** (`keywords` +
   `photo_keywords`-Join-Tabelle), keine Hierarchie/Synonyme/Auto-
   Vervollständigung (siehe ADR-0022 — das ist auf Phase 6 verschoben).
3. **Sammlungen sind in Phase 3 rein manuell** (`collections` +
   `collection_photos` mit fester `position`-Reihenfolge), keine
   Sammlungssätze oder intelligenten Sammlungen mit Regeln (Phase 6).
4. **`photos_fts` ist eine FTS5-External-Content-Virtualtabelle** über
   `photos` (referenziert die Originalspalten statt sie zu duplizieren),
   mit `INSERT`/`UPDATE`/`DELETE`-Triggern auf `photos`, die den Index
   automatisch synchron halten — das ist SQLite FTS5s empfohlenes
   Standardmuster für Volltextsuche über bereits vorhandene Tabellen,
   vermeidet doppelte Datenhaltung und manuelle Sync-Logik in Rust.

**Konsequenzen:** Migration 3 bleibt rein additiv zu den Migrationen 1/2
(keine bestehende Spalte/Tabelle wird geändert). Die FTS5-Trigger sind
reines SQL in der Migrationsdatei, keine zusätzliche Rust-Logik zum
Index-Pflegen nötig.

---

## ADR-0024: Rasteransicht und Filmstreifen teilen sich Fotoliste + Auswahl-State

**Status:** Angenommen
**Kontext:** Phase 3 bringt eine neue Rasteransicht (`GridView.tsx`)
neben dem bestehenden Filmstreifen (`Filmstrip.tsx`, Phase 1). Beide
zeigen dieselbe Fotoliste des aktuell gewählten Ordners/Sammlung/Filters
und brauchen Mehrfachauswahl (für Bewertung/Flagge/Sammlung-Hinzufügen
als Stapel-Aktion).

**Entscheidung:** Fotoliste und Auswahl-State (inkl. Mehrfachauswahl per
Shift/Strg-Klick) leben im gemeinsamen Zustand-Store (`store/index.ts`),
nicht dupliziert in beiden Komponenten. Beide Ansichten nutzen dieselbe
Virtualisierungsbibliothek `@tanstack/react-virtual` (schon Abhängigkeit
seit Phase 1), nur mit unterschiedlichem Layout (1D-Reihe vs. 2D-Raster).

**Konsequenzen:** Ein Foto in einer der beiden Ansichten auszuwählen,
spiegelt sich sofort in der anderen wider — kein Synchronisationscode
nötig, weil es nur einen State gibt.

---

## ADR-0025: Import-Kopieren/Verschieben additiv zum bestehenden Hinzufügen, DNG-Konvertierung verschoben

**Status:** Angenommen
**Kontext:** Phase 1s Import scannt Dateien nur an ihrem bestehenden Ort
("Hinzufügen", `ImportMode::AddInPlace` implizit). `SPEC.md` §5 nennt für
Phase 3 zusätzlich Kopieren/Verschieben in einen verwalteten Zielordner.
DNG-Konvertierung (RAW → DNG beim Import) ist in §3.1 Teil desselben
Aufzählungspunkts, aber nicht in §5s Phase-3-Satz namentlich erwähnt und
technisch ein eigenständiges Feature (braucht einen DNG-Writer, den es im
Projekt noch nicht gibt).

**Entscheidung:** `ImportMode { AddInPlace, Copy(PathBuf), Move(PathBuf) }`
als neuer, additiver Parameter — der bestehende Scan-/Metadaten-/
Thumbnail-Ablauf (`import::run`) ändert sich für `AddInPlace` nicht.
DNG-Konvertierung verschoben auf Phase 5 (Export/Publish), siehe
ADR-0022.

**Konsequenzen:** Keine Verhaltensänderung für bestehende Aufrufer, die
weiterhin implizit `AddInPlace` nutzen.

## ADR-0026: Suche/Filter alternativ statt kombiniert; zwei Über-Scope-Punkte, die ADR-0022 übersehen hatte, nachträglich korrigiert

**Status:** Angenommen
**Kontext:** Bei der Abnahme von Phase 3 (Schritt 7) fielen zwei Lücken
auf, die bei der Scope-Korrektur in Schritt 0 (ADR-0022) hätten
mitkorrigiert werden müssen, es aber nicht wurden:

1. `FEATURES.md` §3.1 hatte "Filterleiste (Text, Attribut, Metadaten,
   **kombiniert**)" auf Phase 3 getaggt — `SPEC.md` §5s Phase-3-Satz
   nennt nur "Filter", ohne das Kombinieren von Text- und
   Attributsuche zu verlangen. Das tatsächlich gebaute
   `FilterBar.tsx`/`store/index.ts` implementiert Freitextsuche
   (`search_photos`, FTS5) und Attributfilter (`filter_photos`,
   inklusive eines nachträglich ergänzten Kameramodell-Chips) als
   **Alternative** statt als kombinierbare UND-Verknüpfung: Setzen des
   einen leert das andere. Eine echte Kombination bräuchte entweder
   eine dritte Backend-Route, die FTS5-`MATCH` und die
   Attribut-`WHERE`-Klausel in derselben Abfrage verknüpft, oder eine
   clientseitige Nachfilterung der Suchergebnisse — beides war im
   verbleibenden Schritt-6-Budget nicht mehr sauber umsetzbar.
2. "Duplikaterkennung per exaktem Hash" und "Sortierung nach beliebigem
   Feld" standen in `FEATURES.md` §3.1 ebenfalls auf Phase 3, kommen
   aber in `SPEC.md` §5s Phase-3-Satz nicht vor — nach genau derselben
   Regel wie ADR-0022 (Phase-3-Satz ist maßgeblich, nicht §3.1s voller
   Katalog) hätten sie schon in Schritt 0 auf eine spätere Phase
   umgetaggt werden müssen. Keins von beiden wurde in Phase 3 gebaut
   (`content_hash` wird beim Import weiterhin nicht berechnet; Raster/
   Filmstreifen sortieren fest nach Dateiname).

**Entscheidung:**
- Suche/Filter bleiben bewusst alternativ (siehe oben) — als
  "Fertig (abweichend, siehe DECISIONS.md)" in `FEATURES.md` markiert,
  nicht als unerledigt liegen gelassen.
- "Duplikaterkennung per exaktem Hash" auf Phase 6 umgetaggt (neben der
  dort bereits stehenden Perceptual-Hash-Duplikaterkennung — beide
  Ausbaustufen derselben Fähigkeit gehören zusammen).
- "Sortierung nach beliebigem Feld" auf Phase 6 umgetaggt (neben
  Filter-Presets/Schnellentwicklung im Raster, mit denen es inhaltlich
  zusammenhängt).

**Weiterer bekannter Stand (kein Umtaggen nötig, aber hier
dokumentiert):** "Ordnerbaum-Synchronisation" ist nur teilweise
gebaut — die Sidebar-Baumdarstellung, `relink_folder` und die
Fehlend-Erkennung (Schritt 5) funktionieren korrekt für jede
`parent_id`, die in der Datenbank steht, aber der Import selbst legt
weiterhin nur den unmittelbaren Elternordner jeder Datei an
(`ensure_folder` in `apx-app/src/import/mod.rs`), ohne die volle
Verzeichniskette bis zum gewählten Import-Ordner als `parent_id`-Kette
nachzubilden — das war so bereits in `PLAN.md` Phase 3 Schritt 5s
Beschreibung angelegt ("Sidebar bekommt eine echte Baumdarstellung"),
nicht in ADR-0022s Scope-Fehler. Eine echte Mehrebenen-Population beim
Import (begrenzt auf den gewählten Import-Ordner, nicht bis zum
Dateisystem-Wurzelverzeichnis) wäre ein sinnvoller kleiner Folgeschritt,
ist aber kein Phase-3-Blocker, da die Fotos unabhängig davon korrekt
einem (flachen) Ordner zugeordnet werden.

**Konsequenzen:** `FEATURES.md` korrigiert (siehe dortige Zeilen);
Definition-of-Done in `PLAN.md` Schritt 7 bewertet diese drei Punkte
entsprechend ehrlich statt sie stillschweigend als erledigt
auszuweisen.

## ADR-0027: Fünf im Abschlussbericht ehrlich benannte Lücken nachträglich in Phase 3 geschlossen

**Status:** Angenommen
**Kontext:** Im Abschlussbericht zu Phase 3 (Schritt 7) wurden fünf Lücken
offen benannt statt stillschweigend übergangen — DoD-Kriterium 5 aus
`SPEC.md` §7 (Undo/Redo) sowie die beiden ADR-0026-Umtaggungen
(Duplikaterkennung, Sortierung), die bewusst alternative statt
kombinierte Suche/Filter, und die teilweise Ordnerbaum-Population beim
Import. Auf ausdrücklichen Nutzerwunsch werden **genau diese fünf
Punkte** jetzt nachgezogen — nicht der komplette restliche
BIBLIOTHEK-Katalog aus `SPEC.md` §3.1 (Gesichtserkennung, Stapel,
Schlagwort-Hierarchie, intelligente Sammlungen, Metadaten-Presets/
Batch-Editing, voller EXIF/IPTC/XMP-Editor + Sidecar-Export,
Katalog-Backup/-Reparatur/-Merge, Perceptual-Hash-Duplikate,
DNG-Konvertierung, Cheatsheet-Overlay bleiben wie in ADR-0022 auf ihre
jeweils spätere Phase verteilt).

**Entscheidung, je Punkt:**

1. **Undo/Redo für Bibliotheks-Metadaten** (Bewertung/Flagge/Farbe/
   Schlagworte/Sammlungsmitgliedschaft): reines Frontend-Feature, keine
   neue Backend-Verlaufstabelle — anders als beim Entwickeln-Verlauf
   (`edit_history`, ADR-0014) gibt es hier keinen natürlichen Ort für
   einen Verlauf im Katalog; die Wahrheit bleibt der zuletzt bekannte
   Frontend-Zustand. Neu `frontend/src/lib/undoStack.ts` (reine,
   getestete Stack-Logik: `pushUndo`/`undo`/`redo`, neuer Push verwirft
   die Redo-„Zukunft", gleiches Prinzip wie ADR-0014), `store/index.ts`
   bekommt `libraryUndoStack`/`libraryRedoStack` plus
   `undoLibraryAction`/`redoLibraryAction`; `App.tsx`s globaler
   Tastatur-Handler behandelt Strg/Cmd+Z / Strg/Cmd+Umschalt+Z, aber nur
   wenn das Entwickeln-Panel nicht offen ist (das hat schon seinen
   eigenen lokalen Handler in `DevelopPanel.tsx`). Bewusst **nicht**
   abgedeckt: Sammlung anlegen/umbenennen/löschen (strukturelle
   Aktionen mit unklarer Undo-Semantik bei Neuvergabe der ID). Bekannter
   Grenzfall bei „Zu Sammlung hinzufügen": Rückgängig entfernt alle
   Fotos, die diese eine Aktion hinzugefügt hat — war eines davon schon
   vorher Mitglied, entfernt Rückgängig es trotzdem (keine
   Mitgliedschafts-Historie pro Foto).
2. **Duplikaterkennung per exaktem Hash**: neue direkte Abhängigkeit
   `sha2` (schon transitiv im `Cargo.lock` vorhanden, Version 0.10.9,
   `MIT OR Apache-2.0`, damit kein neues Lizenzrisiko). Jede beim Import
   gestagte Datei bekommt einen per Streaming berechneten
   SHA-256-Hex-Digest (`BufReader` + `std::io::copy` direkt in den
   Hasher, kein Volleinlesen) in `photos.content_hash` — die Spalte
   existierte seit Phase 1, war aber immer `NULL` (ihr Migrations-
   Kommentar nennt noch ein ursprünglich angedachtes xxHash-Teilhash-
   Schema aus der Phase-1-Planung; das wird hier bewusst nicht
   nachgebaut, Migrationen werden nie nachträglich geändert). Neue
   `Catalog::list_duplicate_photo_groups()` gruppiert nach `content_hash`
   (nur Fotos mit gesetztem Hash, `HAVING COUNT(*) > 1`); `run_with_mode`
   ruft sie am Ende auf und meldet die Gesamtzahl betroffener Fotos über
   ein neues `ImportFinishedPayload.duplicate_count`-Feld. Duplikate
   werden nur angezeigt (Header-Text nach Import, "Duplikate
   anzeigen"-Knopf in der Filterleiste) — der Import selbst wird dadurch
   nicht blockiert oder verändert.
3. **Sortierung nach beliebigem Feld**: bewusst client-seitig
   (`frontend/src/lib/sortPhotos.ts`) statt als weiterer
   SQL-`ORDER BY`-Parameter durch mehrere Backend-Abfragen
   durchgereicht — die komplette Fotoliste ist wegen der Virtualisierung
   (Raster/Filmstreifen) ohnehin schon im Speicher, serverseitiges
   Sortieren brächte keinen Zusatznutzen. `PhotoDto` bekommt dafür ein
   neues `file_size`-Feld (bisher nur intern im Katalog vorhanden).
   Felder: Dateiname (Default, entspricht dem bisherigen impliziten
   Verhalten), Aufnahmedatum, Bewertung, Dateigröße, Kameramodell;
   fehlende Werte sortieren immer ans Ende, unabhängig von der Richtung.
4. **Kombinierte Suche + Filter**: `repository/search.rs` bekommt einen
   gemeinsamen Klausel-Baukasten `build_filter_clause` (aus
   `filter_photos` herausgezogen, das unverändert bestehen bleibt) und
   eine neue `search_and_filter_photos` — mit Suchtext ein FTS5-`MATCH`
   plus die Kriterien-Klauseln per UND, ohne Suchtext identisch zu
   `filter_photos`. Additiv: `search_photos`/`filter_photos` (Commands
   und `Catalog`-Methoden) bleiben zusätzlich bestehen. Die frühere
   ADR-0026-Entscheidung "bewusst alternativ" ist damit zurückgenommen.
5. **Volle Ordnerbaum-Hierarchie beim Import**: begrenzt auf den
   gewählten Import-Ordner (bzw. bei Copy/Move auf den *Zielordner* —
   dort leben die Dateien danach, nicht mehr im Quellordner) — nicht bis
   zum Dateisystem-Wurzelverzeichnis. `ensure_folder` legt jetzt
   rekursiv alle Elternordner zwischen dieser Hierarchie-Wurzel
   (inklusive, `parent_id = NULL`) und dem unmittelbaren Elternverzeichnis
   der Datei an; liegt ein Verzeichnis unerwartet nicht unter der
   Hierarchie-Wurzel (z. B. ein aus dem Baum herausführender Symlink),
   bekommt es defensiv `parent_id = NULL` statt weiter zu rekursieren.
   Da Copy/Move-Zielordner aktuell immer flach bleiben (keine
   Unterordner-Struktur wird beim Kopieren/Verschieben nachgebildet),
   wirkt sich das in der Praxis nur auf `AddInPlace`-Importe mit
   verschachtelten Unterordnern aus.

**Konsequenzen:** `FEATURES.md` markiert "Duplikaterkennung per exaktem
Hash" und "Sortierung nach beliebigem Feld" wieder als Phase 3/Fertig,
die Filterleisten-Zeile von "abweichend, alternativ" auf "Fertig
(kombiniert)"; `PLAN.md` bekommt einen neuen Abschnitt "Schritt 8 —
Nachtrag"; `THIRD_PARTY.md` bekommt einen neuen `sha2`-Eintrag. Alle
übrigen, größeren zurückgestellten Punkte aus ADR-0022 bleiben
ausdrücklich auf ihrer jeweils späteren Phase.

## ADR-0028: Phase-4-Scope präzisiert — Workflow-Punkte verschoben, Objektivprofile und Reparatur bewusst vereinfacht

**Status:** Angenommen
**Kontext:** `SPEC.md` §5s Phasenplan-Satz für Phase 4 nennt namentlich
zehn Werkzeugkategorien: „Kurven, HSL, Farbmischer, Color Grading,
Details, Objektivkorrekturen, Effekte, Kalibrierung, Crop/Geometrie,
Reparatur." `FEATURES.md` §3.4 („Modul ENTWICKELN — Workflow") taggt
zusätzlich acht Punkte (Schnappschüsse, Vorher/Nachher, Einstellungen
kopieren/einfügen, Vorherige übernehmen, Synchronisieren, Auto-Sync,
Referenzansicht, Soft-Proof) als Phase 4 — dieselbe Art Diskrepanz
zwischen dem maßgeblichen §5-Satz und `FEATURES.md`s eigener, weiter
gefasster Interpretation, die schon ADR-0011 und ADR-0022 für frühere
Phasen korrigiert haben.

Zusätzlich verlangen zwei der zehn §5-Kategorien laut `SPEC.md` §3.2
Funktionalität, die ohne mehrwöchige Vorarbeit bzw. externe Testdaten
nicht seriös umsetzbar ist: eine echte, Adobe-LCP-kompatible
Objektivprofil-Datenbank („Datenbank mit Profilen, eigene Profile
importierbar") und echte Computer-Vision-Algorithmen für die
Reparatur-Funktion („Auto-Quellenfindung", „Inhaltsbasiertes Füllen für
größere Bereiche", vergleichbar mit Photoshops Content-Aware
Fill/PatchMatch). Diese Entscheidung wurde dem Nutzer explizit zur Wahl
vorgelegt (nicht stellvertretend getroffen wie bei ADR-0011), da sie den
Umfang stärker einschränkt als eine reine Nachschlage-Korrektur.

**Entscheidung (vom Nutzer bestätigt):**

1. **Workflow-Punkte verschoben:** alle acht `FEATURES.md`-§3.4-Zeilen
   wandern auf Phase 6, wo laut `ARCHITECTURE.md` §7 bereits das
   Maskensystem als fortgeschrittenes Entwickeln-Feature geplant ist.
   Phase 4 umfasst ausschließlich die zehn im §5-Satz genannten
   Werkzeugkategorien plus die per ADR-0011 bereits zugewiesenen
   Grundeinstellungs-Ergänzungen (WB-Pipette, WB-Kamera-Presets, Textur,
   Klarheit, Dunst entfernen, Dynamik, Sättigung).
2. **Objektivkorrekturen:** eigenes, minimales Profilformat
   (handgepflegtes JSON, wenige Beispielprofile, Zuordnung per
   EXIF-Objektiv-/Kamerastring) statt einer echten Adobe-kompatiblen
   Profildatenbank. Die vollen manuellen Regler (chromatische
   Aberration automatisch und manuell, Vignettierung, Verzeichnung,
   Perspektive/Upright, manuelle Transformation) werden trotzdem
   komplett gebaut — nur der *Import fremder Profile* (Adobe LCP/DNG
   Lens Profile) entfällt und wird auf eine spätere Phase verschoben.
   Aus demselben Grund entfällt in der Kalibrierung der DCP-Import für
   Kameraprofile; eine kleine eingebaute Profilliste bleibt.
3. **Reparatur:** manuelles Klonen/Reparieren (Pinsel mit Quellpunkt,
   Radius, Deckkraft, weicher Kante; Reparieren per vereinfachtem
   nahtlosen Überblenden, nicht echtem Poisson-Blending) ist die
   Phase-4-Basis. Auto-Quellenfindung und echtes inhaltsbasiertes Füllen
   für größere Bereiche werden auf Phase 6 verschoben (analog zur
   Perceptual-Hash-Duplikaterkennung, die in ADR-0022 aus demselben
   Grund — fortgeschrittener Algorithmus ohne unmittelbaren
   Minimal-Nutzen — zurückgestellt wurde).
4. **Weitere bewusste Vereinfachungen, die aus denselben Gründen
   zusammen mit den obigen drei Punkten entschieden wurden:** der
   „Guided"-Upright-Modus bekommt 2 statt bis zu 4 Linienpaare;
   „Auto-Ausrichtung am Horizont" nutzt nur die EXIF-Orientierung statt
   eines echten Kantenerkennungs-Verfahrens. Beide sind CV-artige
   Detailfragen derselben Kategorie wie Punkt 2/3, keine eigenständige
   Entscheidung.
5. **Struktur:** Phase 4 bleibt eine durchgehende Phase mit
   feingranularen Schritten (0–13, siehe `PLAN.md`) statt in Unterphasen
   4a/4b gesplittet — die architektonische Zweiteilung (per-Pixel-Modell
   vs. Nachbarschafts-/größenverändernde Operationen) spiegelt sich in
   der Schrittfolge (Schritt 2 legt die Infrastruktur für beide Modelle
   an, spätere Schritte nutzen sie je nach Werkzeug), nicht in getrennten
   Phasen mit eigener Abnahme.

**Konsequenzen:** `FEATURES.md` §3.4 komplett auf Phase 6 umgetaggt;
§3.2s Objektivkorrekturen-/Kalibrierung-/Reparatur-/Geometrie-Abschnitte
bekommen erklärende Kommentare zu den vier Vereinfachungen aus Punkt
2–4, die einzelnen Zeilen bleiben bis zur tatsächlichen Umsetzung auf
„Nicht begonnen" (Auto-Quellenfindung/Inhaltsbasiertes Füllen direkt auf
Phase 6 umgetaggt, da sie in Phase 4 überhaupt nicht gebaut werden).
`PLAN.md` bekommt den neuen Abschnitt „Aktuelle Phase: Phase 4" mit der
Schrittfolge 0–13. Ein echter Adobe-Profil-/DCP-Import sowie
Auto-Quellenfindung/Content-Aware-Fill bleiben in `FEATURES.md` als
offene Phase-6-Punkte sichtbar, nicht stillschweigend fallen gelassen.

**Nachtrag (Schritt 12, Reparatur-Umsetzung):** zwei weitere kleine
Vereinfachungen aus derselben Kategorie wie Punkt 4 (CV-artige
Detailfragen ohne unmittelbaren Minimal-Nutzen), zusammen mit der
tatsächlichen Umsetzung entschieden:
- **Pfad-Ausdünnung:** jeder Strich ist auf `MAX_PATH_POINTS = 32`
  Stützpunkte gedeckelt (`repair.rs`) — das Frontend dünnt einen dicht
  abgetasteten Zeigerpfad beim Loslassen entsprechend aus
  (`RepairOverlay.tsx`s `thinPath`). Bei den Zeigerauflösungen üblicher
  Pinselstriche visuell nicht von einer ungedeckelten Punktzahl zu
  unterscheiden.
- **Live-Vorschau beim Malen ist rein clientseitig:** das SVG-Overlay
  zeigt den gerade gemalten Pfad sofort, der tatsächliche
  Pipeline-Effekt erscheint erst nach dem Loslassen (committeter,
  ausgedünnter Strich) — ein voller Entwickeln-Durchlauf über einen
  wachsenden, noch nicht ausgedünnten Pfad bei jeder Zeigerbewegung wäre
  unnötig teuer.
- **Sensorflecken-Visualisierung** (`FEATURES.md` §3.2) wird ebenfalls
  auf Phase 6 verschoben — sie setzt eine automatische Fleckenerkennung
  voraus (Blob-/Kantenerkennung auf dem Bild), dieselbe Kategorie
  Bildverarbeitungsaufgabe wie die bereits zurückgestellte
  Auto-Quellenfindung, ohne die sie nur eine leere Overlay-Hülle wäre.

## ADR-0029: Schritt 2 baut nur die Grundeinstellungs-Regler + die GPU-Dispatch-Grundlage; Kurven/HSL/Farbmischer/Color-Grading/Kalibrierung/Effekte bekommen ihren eigenen Shader in ihrem eigenen Schritt

**Status:** Angenommen
**Kontext:** Der ursprüngliche Plan für Schritt 2 sah vor,
`stages/basic_fused.wgsl`/`.rs` sofort um praktisch alle verbleibenden
1:1-/positions-bewussten Werkzeugkategorien zu erweitern (Kurven, HSL,
Farbmischer, Color Grading, Kalibrierung, Vignette, Körnung) — bevor
deren Frontend-Verträge (welche Felder, welche Interaktion) in ihren
jeweils eigenen Schritten (4–10) überhaupt feststehen. Das würde
`SPEC.md` §6s Grundsatz „jede Operation ein eigenes Modul mit eigenem
Shader, eigenem Test" verletzen (ein einziges Mega-Modul für acht
verschiedene Werkzeuge) und spekulative Arbeit an Formeln leisten, die
noch keinen Abnehmer haben.

**Entscheidung:** Schritt 2 liefert nur:
1. Die Prüfung/Bestätigung, dass `gpu/dispatch.rs::run_compute_f32`
   unverändert sowohl positions-bewusste als auch nachbarschafts-fähige
   Operationen trägt (Breite/Höhe als zusätzliche `Params`-Felder,
   uneingeschränkter Lesezugriff auf den gesamten Eingabepuffer im
   WGSL-Shader) — keine Änderung an `dispatch.rs` selbst nötig.
2. Die letzten fünf der zwölf Grundeinstellungs-Regler
   (Dunst&nbsp;entfernen/Dynamik/Sättigung in `basic_fused`,
   Textur/Klarheit im neuen, eigenen `stages/local_contrast.{rs,wgsl}` —
   Letztere brauchen echten Nachbarschafts-Zugriff, siehe dessen
   Moduldoku) — damit sind alle zwölf Regler aus `SPEC.md`s
   „Grundeinstellungen" fertig.

Kurven, HSL, Farbmischer, Color Grading, Kalibrierung und Effekte
bekommen stattdessen je ein eigenes Modul mit eigenem Shader und
eigenem GPU/CPU-Paritätstest in ihrem jeweils eigenen Schritt (4–10),
genau wie die sieben Phase-2-Regler es taten — nur der interaktive
Vorschau-Pfad fasst sie am Ende jeweils per ADR-0017-Muster zu einem
Dispatch zusammen, sobald ihre Formel feststeht.

**Konsequenzen:** `PLAN.md`s Schritt-2-Checkliste wird auf die zwei
oben genannten Punkte präzisiert; die „GPU/CPU-Paritätstests je neuem
Teil-Feature"-Zeile bezieht sich nur noch auf Dunst
entfernen/Dynamik/Sättigung/Textur/Klarheit (bereits erledigt), nicht
mehr auf die acht später gebauten Kategorien. Die
Kurven-Sequenzierungsfrage (Farbraum-Konvertierung ins WGSL verlagern
oder Kurven als CPU-LUT-Nachschritt) verschiebt sich entsprechend auf
Schritt 4, wenn Kurven tatsächlich gebaut werden.

## ADR-0030: Objektivkorrekturen (Schritt 9) — Ausgabegröße bleibt unverändert, Perspektive/Upright „Auto"/„Level"/„Vertical"/„Full" bleiben No-op-Platzhalter

**Status:** Angenommen
**Kontext:** `PLAN.md`s Architektur-Grundsatz sah für Objektivkorrekturen
eine „größenverändernde" GPU-Dispatch-Form vor (Ausgabepuffer ≠
Eingabepuffer). Beim tatsächlichen Bau von
`stages/lens_corrections.{rs,wgsl}` zeigte sich: Verzeichnung, CA,
Vignette-Korrektur und manuelle Transformation lassen sich vollständig
als inverse Abbildung mit bilinearer Abtastung ausdrücken, bei der
Ausgabe- und Eingabegröße identisch bleiben (Randpixel werden
geklemmt statt schwarz gefüllt oder automatisch zugeschnitten) — echtes
Zuschneiden auf den gültigen Bildbereich ist ohnehin Aufgabe von
Schritt 11s separatem Geometrie-Werkzeug (das laut Plan bereits als
eigener „CPU-seitiger Crop+Rotate+Resample als letzter Schritt in
`render_rgba8`" vorgesehen ist). Eine echte größenverändernde
Dispatch-Form für Schritt 9 zu bauen wäre doppelte Arbeit gewesen, die
Schritt 11 ohnehin leisten muss.

Zweitens: Die SPEC.md-Perspektive/Upright-Modi „Auto", „Level",
„Vertical" und „Full" setzen in echten Bildbearbeitungsprogrammen eine
automatische Kantenerkennung zur Fluchtpunkt-Bestimmung voraus — eine
CV-Aufgabe vergleichbar mit der in ADR-0028 bereits für Schritt 11s
Auto-Ausrichtung zurückgestellten Kantenerkennung, ohne die dafür
nötigen Bildverarbeitungsbausteine (Kantendetektion, Linienerkennung,
Homografie-Schätzung) im bisherigen Stack.

**Entscheidung:**
1. Objektivkorrekturen liefern die geometrische Abbildung als
   Ein-/Ausgabepuffer-gleich-großen inversen Warp mit bilinearer
   Abtastung (keine größenverändernde Dispatch-Form in Schritt 9 —
   die bleibt für Schritt 11 reserviert).
2. Die Upright-Modi „Auto"/„Level"/„Vertical"/„Full" sind im EDL/UI
   wählbar, tragen aber aktuell keine Wirkung (dokumentierter
   No-op-Platzhalter). „Guided" (die einzige Nutzer-gesteuerte Variante)
   nutzt die ersten zwei markierten Hilfslinien und errechnet daraus
   eine einfache gemittelte Dreh-Korrektur — keine echte
   Mehrlinien-Fluchtpunkt-Homografie.

**Konsequenzen:** `FEATURES.md` markiert die vier automatischen
Upright-Modi als „Fertig (abweichend, siehe ADR-0030)" statt als volle
Automatik. Eine echte Kantenerkennungs-/Homografie-basierte
Upright-Automatik sowie ein echtes Zuschneiden auf den gültigen
Bildbereich nach der geometrischen Korrektur bleiben auf eine spätere
Phase verschoben (zusammen mit den bereits in ADR-0028 zurückgestellten
Punkten: echter Adobe-Profil-Import, Auto-Quellenfindung/
Content-Aware-Fill, echte Auto-Horizont-Kantenerkennung).

## ADR-0031: Phase-5-Scope präzisiert — Preset-Grundlagen jetzt, KI-Generator/Adobe-Interop/Templates-Unterabschnitt verschoben; kein eigenes `apx-presets`-Crate

**Status:** Angenommen
**Kontext:** `SPEC.md` §5 nennt für Phase 5 wörtlich nur „Preset- und
Template-System" — `SPEC.md` §3.5 (der komplette Feature-Katalog dahinter)
ist aber mit Abstand der am weitesten in die Zukunft greifende Abschnitt
des ganzen Dokuments: er setzt eine echte LLM-Anbindung (Anthropic API,
API-Key-Verwaltung), eine numerische Referenzbild-Optimierung über die
Pipeline-Parameter, vollständige Adobe-`.xmp`/`.lrtemplate`-Kompatibilität
in beide Richtungen, und — beim „Templates"-Unterabschnitt — mehrere
Subsysteme voraus, die schlicht noch nicht existieren: `ARCHITECTURE.md`
§7 reserviert die Export-Engine explizit erst für Phase 8–9. Ein
„Export-Template" jetzt zu bauen hieße, eine Konfigurationsoberfläche für
ein Subsystem zu bauen, das sie noch gar nicht ansteuern kann — dieselbe
Art von verfrühter Arbeit, die ADR-0029 für Phase 4 bereits vermieden hat
(kein Shader für ein Werkzeug bauen, bevor sein Frontend-Vertrag
feststeht). Genau wie bei ADR-0011/ADR-0022/ADR-0028 gilt: der §5-Satz ist
normativ, der vollständige §3.x-Katalog wird hier scope-präzisiert.

**Entscheidung:**
1. **Preset-Grundlagen** (`SPEC.md` §3.5, erster Unterabschnitt) sind die
   Phase-5-Basis und werden vollständig gebaut: EDL-Teilmengen-Presets
   (Speichern-Dialog mit Checkbox je Einstellungsgruppe), Ordnerhierarchie
   beliebiger Tiefe, Favoriten, Suche/Tags, Preset-Stärke (0–200 %,
   nachträglich änderbar solange kein anderer Edit dazwischen liegt),
   Live-Vorschau (Hover im Bild + Thumbnail in der Liste), Preset-Stapel
   (mehrere nacheinander, Reihenfolge editierbar), eigenes `.apx`-
   Dateiformat-Im-/Export, Versionierung mit Diff-Ansicht, sowie eine
   vereinfachte Fassung der bedingten Presets (siehe Punkt 4).
2. **Preset-Generator (KI)** — LLM-Anfrage, Referenzbild-Modus,
   Variationen-Generator, „Preset aus Bearbeitung lernen" — wird
   komplett auf **Phase 7** verschoben. `ARCHITECTURE.md` §7 reserviert
   diese Phase bereits wörtlich für „`apx-ai` — ONNX-Runtime-Integration,
   LLM-Client für Preset-Generator" — das ist keine neue Zurückstellung,
   sondern nur die Bestätigung der bereits bestehenden Architektur-Planung.
   Ein Referenzbild-Optimierer (Gradientenverfahren über die
   Pipeline-Parameter) ist zudem dieselbe Kategorie CV-/Optimierungs-
   Aufgabe wie die in ADR-0028 zurückgestellten Punkte.
3. **Adobe-`.xmp`/`.lrtemplate`-Import/-Export** wird auf eine spätere
   Phase verschoben — derselbe Grund wie beim Objektivprofil-/DCP-Import
   in ADR-0028 (kein Adobe-Format-Reverse-Engineering ohne Testdaten als
   eigenständiges Mammutprojekt neben dem eigentlichen Feature). Das
   eigene `.apx`-Format deckt Im-/Export vollständig ab.
4. **Bedingte Presets** werden bewusst vereinfacht gebaut statt ganz
   zurückgestellt (echte, nicht nur angetäuschte Fähigkeit ohne externe
   Abhängigkeit): eine feste, kleine Liste vergleichbarer Metadatenfelder
   (ISO, Blende, Brennweite, Kameramodell, Objektiv — alle bereits in
   `photos` gespeichert), UND-verknüpfte Regeln (Feld, Operator, Wert),
   **kein** UI-Builder für eine freie Bedingungssprache mit ODER/
   Verschachtelung.
   **Nachtrag (Schritt 7):** jede Regel trägt zusätzlich ein optionales
   `section`-Feld — `null` bedeutet „gilt fürs ganze Preset" (Fehlschlag
   verhindert das Anwenden komplett), eine gesetzte Sektion grenzt einen
   Fehlschlag auf genau diese Sektion ein (Rest des Presets bleibt
   wirksam). Ein fehlendes Metadatum am aktuellen Foto (z. B. kein
   EXIF-ISO-Wert) lässt die betroffene Regel konservativ als nicht
   erfüllt gelten, statt sie zu ignorieren.
5. **„Templates" (über Presets hinaus)** — Export-/Wasserzeichen-/
   Metadaten-/Import-/Umbenennungs-/Layout-/Workflow-Templates,
   Template-Marktplatz — wird komplett auf eine spätere Phase verschoben.
   Jedes davon konfiguriert ein Subsystem, das entweder noch gar nicht
   existiert (Export-Engine: Phase 8–9, siehe oben; Layout-Templates für
   Druck/Buch/Diashow/Web: in keinem früheren Phasenplan erwähnt) oder nur
   in Ansätzen (Umbenennungs-Templates: `import::rename` existiert bereits
   rudimentär aus Phase 3, ein vollwertiger Token-Editor bräuchte ohnehin
   erst die anderen Import-Template-Bausteine). Diese Punkte wandern in
   `FEATURES.md` auf die jeweilige Phase, in der ihr zugehöriges
   Subsystem tatsächlich gebaut wird (überwiegend Phase 8–9).
6. **Kein eigenes `apx-presets`-Crate**, obwohl `ARCHITECTURE.md` §7 eines
   vorab benannt hatte: die Phase-5-Basis aus Punkt 1 braucht keine neue
   Pixelverarbeitungslogik — ein Preset ist reine Katalogdaten (Name,
   Ordner, Tags, Bedingungen, eine EDL-*Teilmenge* als opakes JSON) plus
   Frontend-seitiges Zusammenführen/Skalieren in den bestehenden
   `developEdl`-Zustand vor dem ohnehin schon bestehenden
   `commitDevelopEdit`. Genau wie `apx-catalog`s `edit_history.edl_json`
   nie von `apx-catalog` selbst verstanden werden muss (siehe
   `ARCHITECTURE.md` §5), muss auch der Preset-JSON-Blob nie von
   `apx-catalog` verstanden werden — eine neue Erweiterung von
   `apx-catalog` (Migration + Repository-Modul) plus neue `apx-app`-
   Commands reichen. Ein leeres `apx-presets`-Crate jetzt anzulegen wäre
   dieselbe Art von Vorab-Arbeit ohne aktuellen Abnehmer, die ADR-0029
   bereits für Phase 4 vermieden hat. Sollte Phase 7s KI-Generator später
   echte, eigenständige Preset-Berechnungslogik brauchen (Referenzbild-
   Optimierung, Variationen-Generator), ist das der richtige Zeitpunkt,
   ein solches Crate tatsächlich anzulegen.
7. **Import-Templates/Umbenennungs-Templates werden nach Phase 5
   vorgezogen** (waren in `FEATURES.md` fälschlich als „Phase 3"
   getaggt, obwohl Phase 3 bereits abgeschlossen ist, ohne sie zu bauen —
   derselbe Fehlertyp, den ADR-0026/ADR-0027 für andere Zeilen schon
   einmal korrigiert haben). Der Rust-Unterbau existiert bereits
   vollständig aus Phase 3 (`apx-app::import::presets::ImportPreset` +
   die Commands `list_import_presets`/`save_import_preset`/
   `delete_import_preset`), nur ohne jede Frontend-Anbindung — kein
   Import-Dialog nutzt sie, kein Token-Editor existiert. Da beide Punkte
   ohnehin unter „Templates" fallen und ihr Unterbau bereits da ist,
   werden sie als kleiner, eigenständiger Schritt in Phase 5
   mitgenommen statt weiter unbenutzt zu bleiben.

**Konsequenzen:** `FEATURES.md` §3.5 wird zeilenweise auf „Phase 5"
(Preset-Grundlagen + vereinfachte bedingte Presets), „Phase 7" (KI-
Generator) bzw. „Phase 8–9"/„Phase 6" (Templates-Unterabschnitt, je nach
zugehörigem Subsystem) umgetaggt; Adobe-Interop bleibt als offener
späterer Punkt sichtbar. `PLAN.md` bekommt einen neuen Abschnitt „Aktuelle
Phase: Phase 5" mit einer feingranularen Schrittfolge, analog zu Phase 4.
`ARCHITECTURE.md` §7s Phase-5-Platzhalterzeile wird entsprechend
präzisiert (kein `apx-presets`-Crate, stattdessen `apx-catalog`-
Erweiterung + `apx-app`-Commands + Frontend-Merge-Logik).

## ADR-0032: Phase-6-Scope präzisiert — Maskensystem (§5) plus die in ADR-0028 versprochenen Workflow-Punkte; Bibliotheks-Backlog explizit auf Phase 9 verschoben; Tiefenbereich/KI-Masken zurückgestellt

**Status:** Angenommen
**Kontext:** `SPEC.md` §5 nennt für Phase 6 wörtlich nur „Masken und
lokale Anpassungen. Pinsel, Verläufe, Bereichsmasken, Maskenkombination,
Ebenen-Mischmodi." — das deckt sich mit `SPEC.md` §3.3 bis auf zwei
Punkte, die dort zusätzlich stehen: Tiefenbereich-Masken (brauchen
Tiefendaten, die in keiner früheren Phase je erzeugt wurden — keine
Tiefenschätzung, kein Tiefensensor-Import existiert) und die fünf
KI-Masken (Motiv/Himmel/Hintergrund/Objekte/Personen — Segmentierung per
ONNX-Runtime). Letztere sind wörtlich das, was `ARCHITECTURE.md` §7 für
Phase 7 reserviert („`apx-ai` — ONNX-Runtime-Integration"); sie jetzt
vorzuziehen hieße, eine komplette Modell-Inferenz-Pipeline parallel zum
eigentlichen Maskensystem aufzubauen — derselbe Fehler, den ADR-0031 für
den Preset-Generator vermieden hat.

Daneben tragen zwei Gruppen von `FEATURES.md`-Zeilen bereits die Marke
„Phase 6", ohne in `SPEC.md` §5s Phase-6-Satz vorzukommen:
1. Die acht Workflow-Punkte aus §3.4 (Schnappschüsse, Vorher/Nachher,
   Copy/Paste-Einstellungen, Vorherige übernehmen, Sync, Auto-Sync,
   Referenzansicht, Soft-Proof) — diese Zuordnung ist kein Versehen,
   sondern eine **bereits gegebene Zusage**: ADR-0028 hat sie explizit
   „auf Phase 6 verschoben", nicht generisch „auf später". Ein
   Versprechen aus einer bereits abgenommenen Phase ohne neue ADR wieder
   zu brechen wäre derselbe Fehlertyp, den ADR-0026/ADR-0027 an anderer
   Stelle schon einmal korrigiert haben — hier gilt also das Gegenteil:
   die Zusage wird eingehalten.
2. Ein rund ein Dutzend Bibliotheks-Zeilen aus §3.1 (Sammlungssätze/
   intelligente Sammlungen, Stapel, virtuelle Kopien, erweiterbare
   Farbmarkierungen, Schlagworthierarchie/-vorschläge/Tag-Regeln,
   Metadaten-Presets/Stapel-Metadatenbearbeitung/EXIF-IPTC-XMP-Editor/
   Sidecar-Export, Vergleichs-/Übersichtsansicht, Filter-Presets,
   Schnellentwicklung im Raster, Vorschau-Cache-Verwaltung/Smart
   Previews/Offline-Bearbeitung) — anders als Gruppe 1 stammt diese
   Zuordnung aus **keiner** ADR, die Phase 6 beim Namen genannt hätte;
   „Phase 6" war hier nur der nächste freie Platzhalter nach Phase 3s
   Abschluss, kein eingelöstes Versprechen. Alle zusammen wären, grob
   geschätzt, mindestens so viel Aufwand wie das eigentliche
   Maskensystem — sie parallel zu bauen würde Phase 6 auf etwa das
   Dreifache der in `SPEC.md` §5 tatsächlich benannten Arbeit aufblähen.

**Entscheidung:**
1. **Maskensystem-Kern** (`SPEC.md` §3.3, ohne Tiefenbereich/KI-Masken,
   siehe Punkt 3) wird vollständig gebaut: Maskentypen Pinsel, Linearer
   Verlauf, Radialer Verlauf, Farbbereich, Luminanzbereich; Kombination
   per Hinzufügen/Subtrahieren/Schneiden; echte Ebenen-Mischmodi pro
   Maske; Maskenverwaltung (Gruppen, Umbenennen, Ein-/Ausblenden,
   Überlagerungsfarbe, Duplizieren, auf anderes Foto übertragen, als
   wiederverwendbarer Baustein speichern, Kette mit Drag-&-Drop-
   Sortierung).
2. **Pro Maske stehen nur die ton-/farb-/detailbezogenen Werkzeuge zur
   Verfügung** — Grundeinstellungen, Kurven, HSL, Farbmischer, Color
   Grading, Details (Schärfung + Rauschreduzierung) — nicht
   Objektivkorrekturen, Effekte, Kalibrierung, Geometrie oder Reparatur.
   `SPEC.md` §3.3 sagt zwar wörtlich „alle globalen Regler", aber die
   ausgenommenen fünf sind strukturell Ganzbild-Operationen (ein
   Objektiv-Warp oder ein Crop pro Maske separat anzuwenden ergibt
   pipeline-technisch keinen Sinn — beide ändern bereits laut ADR-0030/
   der Phase-4-Pipeline-Reihenfolge die Geometrie des *gesamten* Bildes
   an einer festen Stelle im Ablauf, nicht pro Region) — eine genuine,
   nicht nur vorgetäuschte Fähigkeit für exakt die Werkzeugklasse, für
   die regionale Anwendung tatsächlich Sinn ergibt.
3. **Tiefenbereich-Masken** werden auf eine spätere Phase zurückgestellt
   (kein Tiefendaten-Zulieferer existiert — das wäre eine komplette
   Tiefenschätzungs- oder Sensor-Import-Pipeline als Vorprojekt).
   **KI-Masken** (Motiv/Himmel/Hintergrund/Objekte/Personen) werden auf
   **Phase 7** verschoben — dort baut `apx-ai` ohnehin erstmals die
   ONNX-Runtime-Integration auf, die diese Masken als Erstes brauchen.
4. **Architektur — Ebenenmodell statt Fused-Pass:** die Phase-4-Pipeline
   bleibt unverändert die Grundlage (`render_rgba8`s feste Stufenfolge,
   siehe `ARCHITECTURE.md` §8); Masken laufen danach als neue, letzte
   Gruppe von Stufen, jede Maske sequenziell: (a) Maskenalpha berechnen
   (Pinsel: akkumulierte Stempel-Textur aus den Pinselstrichen, analog
   zu `repair.rs`s Pfad-Uniform; Verläufe: analytische Funktion über
   Position; Farbbereich/Luminanzbereich: Klassifikation pro Pixel), (b)
   Kombinationsregeln mit vorangehenden Masken derselben Gruppe
   anwenden, (c) die in der Maske hinterlegten Werkzeuge auf eine Kopie
   des aktuellen Bildzustands anwenden (derselbe Fused-Pass-Baustein wie
   Phase 4, nur mit den Masken-EDL-Werten statt den globalen), (d)
   alpha-gewichtet mit dem gewählten Ebenen-Mischmodus in den
   Bildzustand zurückmischen. Jede Maske ist damit ein eigener
   Pipeline-Durchlauf — das 16-ms-Ziel wird bei vielen/komplexen Masken
   voraussichtlich Grenzen aufzeigen und ist Gegenstand der
   Abnahme-Schritt-Performance-Nachmessung, genau wie in Phase 2/4.
   **Nachtrag (Schritt 2, beim Bauen entdeckt):** „nach der
   Phase-4-Pipeline" war ungenau — Kurven laufen in der globalen
   Pipeline erst *nach* der Farbraum-Konvertierung auf dem fertigen
   RGBA8-Puffer, während Grundeinstellungen/HSL/Color Grading/Details im
   linearen Arbeitsraum *davor* laufen. Da eine Maske alle sechs
   Werkzeuge in einem Durchlauf anwendet, kann sie nicht an zwei
   Pipeline-Stellen zugleich sitzen. Präzisierung: die gesamte
   Maskenstufe läuft im linearen Arbeitsraum, direkt nach `effects` und
   vor der Farbraum-Konvertierung — Masken-Kurven bekommen dafür eine
   eigene `curves::apply_linear_rgb`-Funktion (dieselbe LUT, angewendet
   auf den linearen statt dem display-referred Tonwert), um eine
   verlustreiche zweite Farbraum-Konvertierung pro Maske zu vermeiden.
5. **EDL-Schema v3** (`crates/apx-pipeline/src/edl/v3.rs`) statt einer
   Erweiterung von `EdlV2` — derselbe Grund wie beim v1→v2-Sprung in
   Phase 4 (`migrate.rs` kennt keine automatische Feldergänzung, siehe
   ADR aus Phase 4 Schritt 1): `masks: Vec<Mask>` ist ein komplett neues
   Feld, `v2_to_v3` hebt bestehende `EdlV2`-Daten unverändert an, `masks`
   startet leer.
6. **Workflow-Punkte** (ADR-0028-Zusage, siehe Kontext) werden
   vollständig gebaut: Schnappschüsse (benannte, klickbare EDL-
   Zwischenstände zusätzlich zum linearen Verlauf), Vorher/Nachher in
   vier Ansichten, Einstellungen kopieren/einfügen mit granularer
   Sektionsauswahl (reine Frontend-Logik auf demselben `PresetEdlSubset`-
   Mechanismus wie Presets — kopiert einfach `developEdl` statt eines
   gespeicherten Presets), Vorherige übernehmen, Synchronisieren über
   beliebig viele ausgewählte Fotos, Auto-Sync-Modus, Referenzansicht,
   Soft-Proof (Zielprofil/Renderpriorität/Farbumfangswarnung/
   Papierweiß — vereinfacht auf die in `apx-pipeline` bereits
   vorhandenen Farbraum-Grundlagen, kein vollständiges ICC-
   Farbmanagement-Subsystem; echte Profilverwaltung wäre ein eigenes
   Mammutprojekt und ist hier nicht das Ziel).
   **Nachtrag (Schritt 8, beim Bauen entdeckt):** „ein Schnappschuss ist
   ein benannter Verweis auf einen bestehenden Verlaufs-Stand" (siehe
   `PLAN.md`s ursprüngliche Formulierung) war unsicher — ein Blick in
   `repository/edits.rs::commit` zeigt, dass jede Bearbeitung nach einem
   Rückgängig die „Zukunft" (Zeilen mit höherer Sequenznummer) hart
   löscht (ADR-0014). Ein Verweis auf so eine Zeile könnte also
   verschwinden, sobald man über einen Schnappschuss hinaus weiterarbeitet
   — das Gegenteil von „zusätzlich zum linearen Verlauf". Präzisierung:
   eine eigene, kleine `snapshots`-Tabelle mit eigener EDL-Kopie je
   Schnappschuss statt eines Verweises — kein Restore-Sonderweg nötig,
   ihr Anwenden ist derselbe `apply_develop_edit`-Aufruf wie jeder andere
   EDL-Stand.

   **Präzisierung (Schritt 10, beim Bauen entschieden):** Soft-Proof
   ("kein vollständiges ICC-Farbmanagement-Subsystem", siehe oben) ist
   als **rein clientseitige Nachbearbeitung** des bereits über die
   bestehende `develop/...`-Route gerenderten RGBA8-Vorschau-Puffers
   umgesetzt (`frontend/src/lib/softProof.ts`), nicht als neue
   Backend-/Pipeline-Stufe — `apx-pipeline` kennt bis heute nur eine
   feste Kamera→sRGB-Matrix plus Gammakurve, kein ICC-Profil-Laden oder
   3D-Gamut-Mapping (`crates/apx-pipeline/src/color/mod.rs`), ein neues
   Backend-Subsystem allein für eine Anzeige-Vorschau wäre unverhältnis-
   mäßig. Zielprofil/Renderpriorität/Farbumfangswarnung/Papierweiß sind
   entsprechend Näherungen (Sättigungs-Kompression Richtung Grauwert,
   Sättigungs-Schwellenwert statt echtem Gamut-Volumen, feste lineare
   Tonwert-Bereichskompression) — siehe `softProof.ts`s Moduldoku für
   Details. Betrifft nie den echten Export, nur die Anzeige.
7. **Bibliotheks-Backlog (Gruppe 2, siehe Kontext) wird explizit auf
   Phase 9 verschoben, nicht in Phase 6 mitgenommen.** Keine ADR hat
   Phase 6 für diese Zeilen je zugesagt; sie parallel zum Maskensystem
   und den acht Workflow-Punkten zu bauen würde diese ohnehin schon
   große Phase auf einen Umfang aufblähen, der `SPEC.md` §5s eigentlicher
   Phase-6-Beschreibung nicht mehr entspricht. Phase 9 („Fortgeschrittenes")
   ist bereits der Sammelpunkt für nachgezogene Reife-Themen (Node-
   Editor, Stacking, Tethering, Skript-API) und passt strukturell besser
   als ein weiteres Aufblähen von Phase 6.
8. **Reparatur-Erweiterungen aus ADR-0028** (Auto-Quellenfindung,
   Sensorflecken-Visualisierung, inhaltsbasiertes Füllen) werden von
   ihrer bisherigen `FEATURES.md`-Markierung „Phase 6" auf **Phase 7**
   verschoben. ADR-0028 hatte sie nur generisch „auf eine spätere Phase"
   verschoben, ohne Phase 6 beim Namen zu nennen — dieselbe Situation wie
   bei Gruppe 2, nicht wie bei den acht Workflow-Punkten. Inhaltlich
   passen sie ohnehin besser zu Phase 7: automatisches Erkennen guter
   Quellregionen und echtes inhaltsbasiertes Füllen sind PatchMatch-
   artige Bildanalyse-Algorithmen, dieselbe CV-Kategorie wie die in
   ADR-0031 nach Phase 7 verschobene Referenzbild-Optimierung.

**Konsequenzen:** `FEATURES.md` §3.3/§3.4-Zeilen werden auf „Phase 6"
bestätigt bzw. bei Tiefenbereich/KI-Masken auf „zurückgestellt"/„Phase 7"
korrigiert; die rund ein Dutzend Bibliotheks-Zeilen aus Gruppe 2 werden
von „Phase 6" auf „Phase 9" umgetaggt. `PLAN.md` bekommt einen neuen
Abschnitt „Aktuelle Phase: Phase 6" mit feingranularer Schrittfolge,
analog zu Phase 4/5. `ARCHITECTURE.md` §7s Phase-6-Platzhalterzeile wird
entsprechend präzisiert (Ebenenmodell statt Fused-Pass, EDL v3, kein
Tiefenbereich/keine KI-Masken).

## ADR-0033: Phase-7-Scope präzisiert — KI-Funktionen ohne echte ONNX-Runtime-Modellinferenz, echter LLM-Client für den Preset-Generator, neues `apx-ai`-Crate

**Status:** Angenommen
**Kontext:** `SPEC.md` §5 nennt für Phase 7 wörtlich „KI-Funktionen.
Motiv-/Himmel-/Personen-Segmentierung (ONNX-Runtime, Modelle lokal),
Preset-Generator per LLM, Referenzbild-Matching, Auto-Tagging."
`ARCHITECTURE.md` §7 reservierte dafür bereits wörtlich „`apx-ai` —
ONNX-Runtime-Integration ..., LLM-Client für Preset-Generator". Zwei
Teile dieses Satzes sind in dieser Umgebung nicht wie ursprünglich
vorgesehen umsetzbar:

1. **Echte ONNX-Runtime-Modellinferenz** würde (a) das Bundling
   tatsächlicher, lizenzrechtlich einwandfreier Segmentierungs-Modell-
   gewichte voraussetzen — es gibt in dieser Umgebung keinen legitimen
   Weg, ein trainiertes Motiv-/Himmel-/Personen-Segmentierungsmodell zu
   beschaffen und mitzuliefern —, und (b) eine native
   `onnxruntime`-Bibliothek zur Build-Zeit verlinken, was in dieser
   Sandbox nicht zuverlässig testbar ist (kein bestätigter Zugriffsweg
   auf vorkompilierte ONNX-Runtime-Binaries). Ein „Bring-your-own-Model"-
   Pfad (Nutzer liefert eine `.onnx`-Datei) wäre technisch denkbar, aber
   ohne jedes mitgelieferte oder in dieser Sandbox verifizierbare Modell
   nur eine ungetestete Hülle — dieselbe Art vorgetäuschter statt echter
   Fähigkeit, die dieses Projekt durchgehend vermeidet (siehe z. B.
   ADR-0032 Punkt 4: „eine genuine, nicht nur vorgetäuschte Fähigkeit").
2. **Ein echter LLM-Client** ist dagegen sehr wohl echt umsetzbar: ein
   HTTP-Client gegen die Anthropic-Messages-API (`reqwest`, vom Nutzer
   selbst hinterlegter API-Schlüssel, genau wie jede andere Desktop-App
   mit KI-Anbindung) ist eine Handvoll Code, braucht kein mitgeliefertes
   Modell und lässt sich isoliert testen (Prompt-Aufbau/Antwort-Parsing
   ohne echten Netzwerkaufruf, analog zum bestehenden „kein GPU-Adapter
   verfügbar"-Überspringen-Muster für Netzwerk-freie CI-Läufe).

**Entscheidung:**
1. **Neues `apx-ai`-Crate** (`crates/apx-ai`) bündelt alle in diesem
   Abschnitt beschriebenen Analyse-/Generator-Bausteine — dieselbe
   „ein Crate pro fachlicher Domäne"-Konvention wie `apx-catalog`/
   `apx-pipeline`. Abhängigkeiten: `apx-core`, `apx-raw` (für
   `LinearImage`), `apx-pipeline` (für EDL-Typen + Rendering, nur beim
   Referenzbild-Modus gebraucht), `apx-catalog` (für „Preset aus
   Bearbeitung lernen" + Auto-Tagging, beide lesen bestehende
   Katalogdaten), `reqwest` (LLM-Client, neue Workspace-Abhängigkeit,
   `rustls-tls`-Feature statt der systemabhängigen OpenSSL-Variante).
2. **Die fünf KI-Masken** (Motiv/Himmel/Hintergrund/Objekte/Personen)
   werden über **klassische, deterministische Bildverarbeitungs-
   heuristiken statt echter tiefer neuronaler Netze** umgesetzt — jede
   einzelne ist eine echte, funktionierende, unit-getestete Fähigkeit,
   kein Platzhalter:
   - **Motiv:** Sättigungs-/Kontrast-gewichtete Saliency-Karte
     (Center-Surround-Kontrast, ein reales, jahrzehntealtes klassisches
     Sichtverfahren) als Vordergrund-Wahrscheinlichkeit, geglättet.
   - **Himmel:** Farbton-/Helligkeits-/Positions-Heuristik (bläulich,
     geringer lokaler Kontrast, obere Bildhälfte bevorzugt) — dieselbe
     Art Heuristik, die vor Verbreitung tiefer Netze in echten
     Foto-Editoren für „Himmel auswählen" verwendet wurde.
   - **Hintergrund:** Komplement der Motiv-Maske (`1.0 - alpha`) — kein
     eigener Algorithmus nötig.
   - **Objekte (Klick-Segmentierung):** Region-Growing/Flood-Fill ab dem
     Klickpunkt, farbtoleranz-basiert — dieselbe Toleranz-/Weich-
     zeichnung-Grundidee wie die bereits bestehende Farbbereich-Maske
     (Phase 6 Schritt 5), hier aber ausgehend von einem Saatpunkt statt
     einem global über das ganze Bild verglichenen Zielfarbwert.
   - **Personen:** Hautton-Erkennung im YCbCr-Farbraum (ein reales,
     weit verbreitetes klassisches Verfahren) als **eine einzelne
     zusammenhängende Hautregion** — **bewusste Einschränkung:** die in
     `SPEC.md` §3.3 genannten Einzelregionen (Augen, Brauen, Lippen,
     Zähne, Haare, Kleidung) werden **nicht** als eigene, einzeln
     wählbare Teilmasken umgesetzt — echte Gesichts-/Körper-Landmark-
     Erkennung für diese Feinheit setzt ein trainiertes Modell voraus
     (siehe Punkt 1).
   Jede Heuristik läuft serverseitig (Rust, `apx-ai`), niemals
   clientseitig — konsistent mit dem gesamten Projekt: jede tatsächliche
   Pixelanalyse lebt in Rust, das Frontend liest nur schon gerenderte
   Einzelpixel (WB-Pipette/Farbmischer/Maskenfarbbereich).
3. **Maskengeometrie-Erweiterung:** ein KI-generiertes Ergebnis ist
   naturgemäß eine Rasterfläche, kein Parameter-Satz wie die fünf
   bestehenden Geometrietypen — `MaskGeometry` bekommt eine sechste
   Variante `AiGenerated { kind, width, height, alpha: Vec<u8> }`: eine
   niedrig aufgelöste (lange Kante auf 512px begrenzt) Alpha-Bitmap,
   beim Rendern bilinear auf die tatsächliche Zielauflösung hochskaliert
   — **bewusste Vereinfachung** ggü. den parametrischen Typen (die bei
   jeder Auflösung exakt neu berechnet werden): geringerer
   Speicherbedarf als eine volle Auflösung, aber weniger scharfe Kanten
   bei starker Vergrößerung. Einmal berechnet, bleibt sie bis zu einer
   erneuten Generierung unverändert (kein Re-Run bei jedem Regler-Tick).
4. **Reparatur-Erweiterungen** (aus ADR-0032 Punkt 8 bereits auf Phase 7
   vorgemerkt): Auto-Quellenfindung und Sensorflecken-Visualisierung
   sind einmalige Analyse-Befehle in `apx-ai` (Patch-Ähnlichkeitssuche
   per normierter Kreuzkorrelation bzw. Blob-Erkennung per lokaler
   Kontrast-Anomalie gegen ein weichgezeichnetes Referenzbild) — beide
   echte, deterministische, testbare Algorithmen. Inhaltsbasiertes
   Füllen (`RepairMode::ContentAwareFill`) ist dagegen ein *Render-Zeit*-
   Vorgang (läuft bei jedem Rendering wie Klonen/Reparieren, nicht nur
   einmalig) und bleibt deshalb in `apx-pipeline::stages::repair`, nicht
   in `apx-ai` — umgesetzt als vereinfachtes PatchMatch (Zufallsinit +
   Propagation + Zufallssuche, wenige Iterationen), eine reale, wenn auch
   gegenüber der vollen PatchMatch-Veröffentlichung reduzierte Fassung
   (weniger Iterationen, kein Multi-Skalen-Ansatz).
5. **Preset-Generator:** LLM-Anfrage nutzt einen echten Anthropic-
   Messages-API-Client (`apx-ai::llm_client`), der Nutzer hinterlegt
   seinen eigenen API-Schlüssel in den Einstellungen (`apx-core::Settings`
   bekommt ein neues `AiSettings`-Feld, exakt demselben Speicher-
   /Lademuster wie `UiSettings`/`CatalogSettings"). Der Prompt beschreibt
   dem Modell das EDL-Sektionsschema und bittet um eine JSON-Antwort im
   selben Format wie eine Preset-EDL-Teilmenge — Parsing/Validierung
   passiert serverseitig, eine unparsbare Antwort führt zu einem
   Fehler statt eines stillschweigend übernommenen Unsinns-Presets.
   Referenzbild-Modus (numerische Optimierung, **kein** LLM):
   Koordinatenabstieg über eine kleine, feste Teilmenge der
   Grundeinstellungs-Parameter, Zielfunktion = Histogramm-Distanz
   zwischen gerendertem aktuellen Bild und Referenzbild — bewusst kein
   Gradientenverfahren über die volle Pipeline (nicht differenzierbar
   ohne Autodiff-Infrastruktur), sondern ein einfaches, aber echtes
   Ableitungsfreies Suchverfahren. Variationen-Generator: deterministisch
   geseedete kleine Störungen eines Basis-Presets. Preset aus Bearbeitung
   lernen: Mittelwertbildung der committeten EDL-Werte mehrerer vom
   Nutzer ausgewählter Fotos je EDL-Sektion.
6. **Auto-Tagging** wird bewusst **regelbasiert statt echter
   Bildklassifikation** umgesetzt: leitet aus den KI-Masken-Heuristiken
   aus Punkt 2 (nennenswerte Himmel-/Hautton-Fläche erkannt) plus
   bereits vorhandenen EXIF-Feldern (ISO/Blende/Brennweite/Zeitstempel)
   eine kleine, feste Menge an Schlagwort-Vorschlägen ab (z. B. „Himmel",
   „Person", „Nachtaufnahme", „Makro") — reuse der bestehenden
   Schlagwort-Infrastruktur aus Phase 3 (`photo_keywords`), kein neues
   Schema. **Nachtrag:** „Auto-Tagging" stand im `SPEC.md` §5-Satz für
   Phase 7, hatte aber bislang **keine** eigene `FEATURES.md`-Zeile — eine
   echte Dokumentationslücke aus Phase 3, hier nachgetragen (§3.1, Phase
   7) statt stillschweigend übersprungen.

**Konsequenzen:** `FEATURES.md` §3.3 (fünf KI-Masken-Zeilen) und §3.5
(Preset-Generator-Zeilen) werden von „Nicht begonnen" auf „Fertig
(abweichend)" umgetaggt, sobald gebaut, mit Verweis auf diese ADR; §3.1
bekommt eine neue Auto-Tagging-Zeile. `ARCHITECTURE.md` §7s
Phase-7-Platzhalter wird durch ein neues Kapitel „Architektur Phase 7"
ersetzt. `PLAN.md` bekommt einen neuen Abschnitt „Aktuelle Phase: Phase 7"
mit feingranularer Schrittfolge. Tiefenbereich-Masken (siehe ADR-0032
Punkt 3) bleiben weiterhin ohne Phasenzuordnung zurückgestellt — kein
Tiefendaten-Zulieferer existiert, das ändert auch diese ADR nicht.

## ADR-0034: Phase-8-Scope präzisiert — Export-Engine als gemeinsamer Unterbau, reale Zusatzformate/ICC-Farbmanagement/PDF/SFTP wo machbar, HEIF-/JPEG-XL-Export und Kartenkacheln bewusst zurückgestellt

**Status:** Angenommen
**Kontext:** `SPEC.md` §5 nennt für Phase 8 wörtlich „Export und
Ausgabe-Module. Export-Engine, Warteschlange, Wasserzeichen, dann
Drucken, Diashow, Buch, Web, Karte." `ARCHITECTURE.md` §7 reservierte
dafür bislang nur einen Platzhalter. Mehrere frühere ADRs haben
zusätzlich einzelne `FEATURES.md`-Zeilen ausdrücklich auf Phase 8
vorgemerkt (ADR-0025: DNG-Konvertierung beim Import; ADR-0031 Punkt 5:
Export-/Wasserzeichen-/Metadaten-/Layout-/Workflow-Templates +
Template-Marktplatz-Struktur). Anders als Phase 6/7 ist Phase 8 damit
kein einzelnes fachliches Thema, sondern sechs weitgehend unabhängige
Ausgabe-Module (Export-Engine, Drucken, Diashow, Buch, Web, Karte), die
nur die Export-Engine als gemeinsamen Unterbau teilen. Diese ADR prüft
für jedes Modul, was in dieser Umgebung tatsächlich echt umsetzbar ist
(dieselbe Prüfungspflicht wie ADR-0033 für ONNX-Runtime), bevor
`PLAN.md` die feingranulare Schrittfolge bekommt.

**Vorab durchgeführte Machbarkeitsprüfung** (`cargo add --dry-run`
gegen den echten crates.io-Index, da direkte HTTP-Anfragen an die
crates.io-API in dieser Sandbox durch den Proxy blockiert werden):
`printpdf`, `lcms2`, `suppaftp`, `russh`, `ravif`, `webp`, `libheif-rs`,
`jxl-oxide`, `psd`, `dng`, `quick-xml`, `geoutils` und
`reverse_geocoder` lösen alle erfolgreich auf. Kein System-`ffmpeg`
in dieser Sandbox vorhanden (`command -v ffmpeg` liefert nichts) —
das sagt nichts über die Zielmaschine der Nutzer aus, ist aber
Anlass für die Video-Export-Entscheidung unten.

**Entscheidung** (in der von `SPEC.md` §5 vorgegebenen Reihenfolge):

1. **Export-Engine (gemeinsamer Unterbau).** Neues `apx-export`-Crate
   (dieselbe „ein Crate pro fachlicher Domäne"-Konvention wie
   `apx-catalog`/`apx-pipeline`/`apx-ai`), hängt von `apx-core`/
   `apx-raw`/`apx-pipeline`/`apx-catalog` ab. Rendert über den
   bestehenden `apx_pipeline::develop::render_rgba8`-Pfad (derselbe
   Renderer wie Vorschau/Viewer — kein zweiter Rendering-Codepfad) und
   kodiert dann in das Zielformat.
   - **Formate:** JPEG/PNG/TIFF sind über das bereits vorhandene
     `image`-Crate abgedeckt (bislang nur zum *Lesen* im
     Fallback-Importpfad genutzt, Schreib-Features werden hier neu
     aktiviert). **WebP und AVIF kommen echt dazu** (`ravif` für
     AVIF-Encoding ist reines Rust, kein Systemabhängigkeit). **PSD-,
     HEIF/HEIC- und JPEG-XL-Export werden bewusst zurückgestellt:**
     PSD hat keine ausgereifte Rust-Schreib-Bibliothek (die einzige
     gefundene, `psd`, ist ein Lese-Parser für Spieleentwicklungs-
     Pipelines, kein Encoder); HEIF/HEIC-Encoding braucht einen
     lizenzierten HEVC-Encoder (dieselbe Art Lizenz-/Beschaffungs-
     mauer wie ONNX-Modellgewichte in ADR-0033 Punkt 1 — `libheif-rs`
     bindet zwar an `libheif`, aber ohne einen frei redistribuierbaren
     HEVC-Encoder dahinter bliebe „HEIF-Export" eine Hülle ohne
     echten Encoder); JPEG XL hat aktuell keinen ausgereiften
     reinen-Rust-Encoder (`jxl-oxide`, bereits transitive
     Depenenz von `image` für JPEG-XL-*Decoding*, ist ein Decoder,
     kein Encoder — echtes Encoding bräuchte `libjxl`-Bindungen).
     `FEATURES.md`s Formatzeile wird bei Umsetzung entsprechend in
     „Fertig (abweichend)" mit den drei genannten Ausnahmen
     umgetaggt statt stillschweigend übersprungen.
   - **Farbräume/ICC:** **`lcms2` kommt zurück** (war seit Phase 1 in
     `Cargo.toml`, wurde aber nie verdrahtet und in Phase 2 Schritt 9
     wieder entfernt, siehe `THIRD_PARTY.md` — Farbtransformation kam
     bislang mit einer festen Matrix plus Gammakurve aus, Phase 6s
     Soft-Proof simuliert Zielprofile nur über einen
     Sättigungs-Kompressionsfaktor statt echter Profile). Diesmal mit
     echtem Verdrahtungsgrund: eine *exportierte Datei* muss ihr
     Farbprofil korrekt einbetten (Druckdienstleister/Web-Betrachter
     werten es aus), eine reine Bildschirmvorschau (Soft-Proof) verzeiht
     Ungenauigkeit stärker als eine tatsächlich weitergereichte Datei.
     `lcms2`s `static`-Feature baut Little-CMS aus dem Quellcode statt
     eine System-Bibliothek vorauszusetzen (dieselbe Bundled-statt-
     System-Konvention wie `rusqlite`s `bundled`-Feature). Bundle die
     vier in `SPEC.md` genannten Standardprofile (sRGB/Adobe
     RGB/ProPhoto RGB/Display P3, feste Primärvalenzen + Tonkurve,
     dieselben Daten, die Phase 6s Soft-Proof bereits für seine
     Simulation nutzt) plus einen Dateiauswahl-Dialog für „eigenes
     ICC" (`lcms2::Profile::new_file`). Phase 6s Soft-Proof bleibt
     unverändert simuliert (reine Vorschau, kein Umbau nötig) —
     nur der tatsächliche Datei-Export bekommt echtes Farbmanagement.
   - **Wasserzeichen:** Bild-/Text-Overlay direkt auf dem gerenderten
     RGBA8-Puffer vor der Kodierung — Text-Rendering über eine
     Font-Rasterisierungs-Bibliothek (z. B. `ab_glyph`), kein neues
     Architekturrisiko.
   - **Export-Warteschlange:** dieselbe Fortschritts-/Abbruch-Architektur
     wie der bestehende Import-Job (Phase 1: `tokio`-Task +
     Tauri-Events fürs Fortschritts-Streaming, `CancellationToken` für
     Pausieren/Abbrechen) — keine neue Konstruktion, reuse.
   - **Import mit DNG-Konvertierung:** die `dng`-Bibliothek existiert
     und löst auf; ihr tatsächlicher Funktionsumfang (reiner Reader
     oder auch Schreibpfad) wird erst bei der Umsetzung geprüft — falls
     sie sich als lese-only herausstellt, wird diese eine Zeile
     zurückgestellt statt das ganze Schritt zu blockieren.
   - **Bit-Tiefe 8/16, Größenbegrenzung, Ausgabeschärfung nach Medium:**
     reine Parametrisierung des bestehenden Renderers, kein neues
     Architekturrisiko.

2. **Drucken.** Reine Layout-/Rastergeometrie (Einzelbild/Kontaktbogen/
   Bilderpaket/benutzerdefiniertes Raster) über der Export-Engine aus
   Punkt 1 — „Speichern als JPEG" ist derselbe Exportpfad mit einem
   Druck-Layout als Eingabe statt eines Einzelbilds. Kein Rust-System-
   Druckertreiber-Zugriff (CUPS/Windows-Druckdialog) in dieser
   Phase — Ausgabe ist eine druckfertige Bilddatei, kein direkter
   Druckauftrag; System-Druckdialog-Integration bliebe, falls je
   gewünscht, eine spätere Erweiterung.

3. **Diashow.** Übergänge/Ken-Burns/Intro-Outro sind reine Frontend-
   Canvas-Wiedergabe (dieselbe Art clientseitige Animation wie
   Phase 6s Vorher/Nachher-Ansicht), kein neues Rust-Crate.
   Musik-Synchronisation braucht kein Rust-Audio-Crate — das
   Tauri-Webview-`<audio>`-Element liefert `duration`/Zeitstempel-
   Events bereits im Browser. **Video-Export (MP4) ruft ein
   System-`ffmpeg` auf, statt eines mitgelieferten Binaries oder
   eines reinen-Rust-Encoders** (für H.264/MP4 gibt es keinen
   praxistauglichen reinen-Rust-Encoder; `ffmpeg` selbst mitliefern
   hieße, pro Zielplattform ein Binary mit eigener Lizenzprüfung
   auszuliefern, siehe `THIRD_PARTY.md`s GPL-Ausnahme-Regel). Beim
   Start wird `ffmpeg -version` per `std::process::Command` geprüft
   — vorhanden: echter Video-Export über einen echten Encoder; fehlt
   es: eine klare, umsetzbare Fehlermeldung („ffmpeg installieren"),
   keine stillschweigend leere Funktion. Dieselbe Holen-und-notfalls-
   ehrlich-scheitern-Vorgehensweise wie diese Umgebung selbst für
   `ANTHROPIC_API_KEY`/Cargo-Registry-Zugriff verwendet.

4. **Buch.** Seitenlayouts/Vorlagen/Text-Stile als datengetriebene
   Layout-Engine (Raster + konfigurierbare Slots), derselbe deklarative
   Aufbau wie das Masken-/Preset-System. **PDF-Export ist echt
   umsetzbar** über `printpdf` (reines Rust, MIT-Lizenz, keine
   System-Bibliothek). Druckerei-Presets sind reine Parametersätze
   (Beschnitt/Farbprofil/Auflösung je Anbieter), keine neue Fähigkeit.

5. **Web.** HTML-/responsiver Galerie-Generator ist serverseitiges
   Rust-Templating (Miniaturbilder über den bestehenden
   `image`-Verkleinerungspfad aus dem Import-Thumbnail-Schritt, Phase 1).
   **FTP/SFTP-Upload ist echt umsetzbar:** `suppaftp` (FTP/FTPS) und
   `russh` + `russh-sftp` (reines Rust SSH/SFTP, keine
   OpenSSL-/libssh2-Systemabhängigkeit, dieselbe Bevorzugung reiner
   Rust-Implementierungen wie `rustls` an anderer Stelle im Projekt).

6. **Karte.** GPS-Auslesen aus EXIF ist eine Erweiterung des
   bestehenden Metadaten-Pfads (`kamadak-exif`/`rawler` lesen GPS-Tags
   bereits mit, bislang nur ungenutzt durchgereicht). **Reverse
   Geocoding bleibt vollständig offline**: `reverse_geocoder` bündelt
   einen GeoNames-Städte-Datensatz und braucht keinen Online-Dienst —
   dieselbe Offline-zuerst-Haltung wie der Rest der App. **Bewusste
   Einschränkung, einzige Ausnahme von „offline zuerst" in dieser
   Phase:** die eigentliche Kartenansicht (Kachel-Bilder) ist ein
   Frontend-Feature (z. B. Leaflet.js gegen OpenStreetMap-Kacheln) und
   braucht zum *Anzeigen* der Hintergrundkarte eine Internetverbindung
   — ein weltweiter Offline-Kachelsatz wäre unverhältnismäßig groß;
   ohne Netzwerk bleiben die GPS-Koordinaten/Ortsschlagworte trotzdem
   nutzbar, nur ohne Kartenbild. GPX-Tracklog-Import ist einfaches
   XML-Parsing (`quick-xml`). Reiserouten-Ansicht leitet sich aus
   GPS-getaggten Fotos nach Aufnahmezeit sortiert ab, reine
   Frontend-Darstellung ohne neues Backend-Risiko.

**Konsequenzen:** `PLAN.md` bekommt einen neuen Abschnitt „Aktuelle
Phase: Phase 8" mit feingranularer Schrittfolge in der oben
begründeten Reihenfolge (Export-Engine zuerst als Unterbau, danach
Drucken/Diashow/Buch/Web/Karte, exakt wie `SPEC.md` §5 sie nennt).
`FEATURES.md`s Phase-8-Zeilen bleiben bis zur jeweiligen Umsetzung
„Nicht begonnen" — keine Korrektur nötig, sie waren bereits vollständig
und korrekt getaggt (inkl. der Export-Warteschlange-Zeile). PSD-/
HEIF-/JPEG-XL-Export und echte System-Druckdialog-Integration bleiben
ohne Phasenzuordnung zurückgestellt, bis eine lizenzrechtlich/technisch
tragfähige Bibliothek verfügbar ist.

## ADR-0035: Phase-9-Scope präzisiert — drei angesammelte Rückstände zusammengeführt, elf „bewusste Vereinfachung"-Grenzen für die technisch schwierigsten Punkte festgelegt, Tiefenbereich-Masken bleiben ausgenommen

**Status:** Angenommen
**Kontext:** `SPEC.md` §5 nennt für Phase 9 wörtlich „Fortgeschrittenes:
Node-Editor, Panorama/HDR/Fokus-Stacking, Tethering, Skript-API,
Plugin-System" (ergänzt um §3.6 „Zusätzliche Module": Astro-Stacking,
Stapelverarbeitungs-Konsole, Vergleichs-Grid, Zeitleisten-Ansicht,
Verlaufs-Vergleich, Kollaborationsmodus — zwölf Punkte in Summe). Über
die Phasen 3–8 hinweg haben mehrere frühere ADRs zusätzlich einzelne
`FEATURES.md`-Zeilen ausdrücklich auf Phase 9 vorgemerkt, statt sie in
der jeweils laufenden Phase mitzubauen:

- **ADR-0032** (Phase-6-Scope) verschob das komplette
  „Bibliotheks-Backlog" aus `SPEC.md` §3.1 nach Phase 9 — vierzehn
  Punkte (Perceptual-Hash-Duplikaterkennung/-Assistent,
  Sammlungssätze/intelligente Sammlungen, Stapel, virtuelle Kopien,
  erweiterbare Farbmarkierungen, Schlagworthierarchie, Metadaten-
  Presets/EXIF-IPTC-XMP-Editor/Sidecar-Export, Vergleichs-/
  Übersichtsansicht, Personenansicht, Filter-Presets, Schnellentwicklung
  im Raster, Vorschau-Cache/Smart Previews, Sekundäres Display,
  Katalog-Statistik-Dashboard) — mit der ausdrücklichen Begründung,
  dass keine ADR diese Punkte je Phase 6 zugesagt hatte und ihre
  Mitnahme Phase 6 auf etwa das Dreifache von `SPEC.md` §5s tatsächlicher
  Phase-6-Zeile aufgebläht hätte.
- **ADR-0031 Punkt 3/5** verschob Adobe-`.xmp`/`.lrtemplate`-Interop
  sowie Export-/Wasserzeichen-/Metadaten-/Layout-/Workflow-Templates
  nach Phase 8–9 (Letztere sind mit Phase 8 Schritt 8 bereits erledigt,
  Erstere bleibt offen — eine Einzelzeile).
- **Ein expliziter Nutzerwunsch** während Phase 8 (Vergleich mit einem
  Lightroom-Classic-Screenshot) ergänzte elf weitere UI-nahe
  Entwickeln-/Anzeige-Fähigkeiten, die weder in `SPEC.md` noch bislang
  in `FEATURES.md` vorkamen (Live-Histogramm, Clipping-Warnungen,
  Punktfarbmesser, Zielgerichtetes Anpassungswerkzeug, Schwarzweiß-
  Mixer, Auto-Ton/Auto-Weißabgleich, Navigator-Miniaturansicht,
  KI-Entrauschung, KI-Hochskalierung, Info-Overlay, Bearbeitungs-Pins)
  — bereits als eigener Backlog-Abschnitt in `PLAN.md`/`FEATURES.md`
  §3.2 vorgemerkt, aber ausdrücklich noch ohne eigene Scope-Präzisierung
  (siehe dortiger Hinweis: „Phase 9 ist noch nicht die aktuelle Phase").

Macht zusammen **38 Einzelpunkte** — mehr als Phase 6+7+8 zusammen. Der
Nutzer hat ausdrücklich gebeten, alle 38 jetzt vollständig zu planen und
umzusetzen, statt weiter aufzuschieben. Diese ADR übernimmt für die
technisch schwierigsten Punkte dieselbe Prüfungspflicht wie ADR-0033
(ONNX-Runtime) und ADR-0034 (Formats-/Bibliotheks-Machbarkeit), bevor
`PLAN.md` die feingranulare Schrittfolge bekommt.

**Vorab durchgeführte Machbarkeitsprüfung** (`cargo add --dry-run`
gegen den echten crates.io-Index bzw. `npm view` gegen den echten
npm-Registry-Index, dieselbe Methode wie in ADR-0034): `rhai` (reines
Rust, sandboxed, aktiv gepflegt), `libloading`, `rustfft`, `imageproc`
und `gphoto2` (LGPL-2.1, bindet an System-`libgphoto2`) lösen alle
erfolgreich auf. `@xyflow/react` (React Flow, aktueller Paketname)
löst im npm-Registry auf. `opencv`-Bindings lösen zwar in Cargo auf,
brauchen aber eine System-OpenCV-Installation, die in dieser Sandbox
nicht vorhanden ist — dasselbe Problemmuster wie ONNX Runtime in
ADR-0033. `libgphoto2` (für Tethering) ist in dieser Sandbox ebenfalls
nicht installiert, und anders als bei `ffmpeg` (ADR-0034 Punkt 3, wo
zumindest ein „ist installiert oder nicht"-Laufzeitcheck möglich ist)
fehlt hier sogar die Systembibliothek zum *Kompilieren* mit aktiviertem
Feature.

**Entscheidung:** Alle 38 Punkte bleiben Phase-9-Scope, aufgeteilt in
zwölf Bauschritte (`PLAN.md` Schritt 1–12) plus diesen Scope-Schritt
(Schritt 0) — Reihenfolge unten richtet sich danach, was aufeinander
aufbaut (Bibliothek vor Stacking, das dieselbe `stacks`/`virtual_copies`-
Schema braucht) und was das geringste Risiko trägt (reine
Analyse-/Anzeige-Erweiterungen zuerst, Architekturrisiko-Punkte
zuletzt). Für die technisch schwierigsten Punkte gilt je eine bewusste
Vereinfachung, die hier bindend festgehalten wird (Details/Begründung
je Punkt in `PLAN.md`s Schritt-Text):

1. **Node-Editor** zeigt und schaltet die bestehende, fest verdrahtete
   Rendering-Reihenfolge (`apx_pipeline::develop::render_rgba8`) —
   keine frei umbaubare oder verzweigende Ausführungsgraph-Engine, weil
   das die Renderpfad-Garantie brechen würde, auf die jedes andere
   Modul (Viewer, Export, Masken) angewiesen ist. Technisch: neues
   `EdlV4`-Schema mit einem `enabled: bool`-Feld pro Pipeline-Stufe,
   `v3_to_v4`-Migration, `@xyflow/react` im Frontend.
2. **Panorama-/Astro-Stacking** beschränken sich in v1 auf reine
   Verschiebungs-Registrierung per 2D-Phasenkorrelation (`rustfft`,
   Stativ-/gleicher-Blickpunkt-Annahme) — echtes merkmalsbasiertes
   Homographie-Stitching für Freihand-/gedrehte Aufnahmen bleibt
   zurückgestellt, weil `opencv` eine fehlende Systembibliothek
   voraussetzt und keine verifizierte, ausgereifte reine-Rust-
   Merkmalserkennungs-plus-RANSAC-Pipeline existiert. Fokus-Stacking
   (Laplacian-Schärfe-Auswahl über bereits ausgerichtete Frames) und
   HDR-Merge (Debevec-Gewichtung + Reinhard-Tonemap) brauchen dagegen
   keine Ausrichtungs-Bibliothek und werden vollständig gebaut.
3. **Skript-API + Plugin-System:** „stabile ABI" heißt eine
   handgepflegte, versionierte `#[repr(C)]`-Funktionszeiger-Tabelle für
   genau einen festen Erweiterungspunkt (eine Bildoperationsstufe) plus
   eine schmale, primitiv-typisierte `rhai`-Skript-API — **keine**
   Zusage, dass beliebige künftige interne Rust-Strukturen dauerhaft
   binärkompatibel bleiben (Rust selbst hat keine stabile ABI). Eine
   Versions-Fehlpassung wird beim Laden hart mit klarer Fehlermeldung
   abgelehnt statt still falsch zu kompilieren.
4. **Kollaborationsmodus** bleibt ein asynchroner Export→Weitergabe→
   Import→Konfliktauflösung-Ablauf über eine neue `.apxt`-artige
   `ApxShareFile` (Metadaten/EDL/Presets, keine Pixel-Bytes) — kein
   Echtzeit-Mehrbenutzer-Modus (kein Live-Cursor/keine Präsenz/kein
   CRDT/keine Netzwerksynchronisation), weil sich Letzteres in dieser
   Sandbox ohne zweiten echten Nutzer und ohne Mehrbenutzer-
   Serverinfrastruktur nicht verifizieren lässt. `apx-catalog` bleibt
   dabei unverändert ein einzelner `Mutex<Connection>` (ADR-0008).
5. **Tethered Shooting:** die `gphoto2`-Anbindung (LGPL-2.1, braucht
   eine `THIRD_PARTY.md`-Ausnahme wie bereits bei `rawler`) wird hinter
   einem standardmäßig ausgeschalteten Cargo-Feature `tethering` real
   geschrieben (`TetherBackend`-Trait, echtes `Gphoto2Backend` plus ein
   `FakeBackend` für alle normalen Tests) — **über ADR-0034s
   ffmpeg-Präzedenzfall hinaus bewusst eingeschränkt:** die echten
   libgphoto2-FFI-Aufrufe sind nie gegen eine echte Kamera oder auch
   nur eine installierte `libgphoto2`-Bibliothek ausführbar, weil
   Letztere in dieser Sandbox und im Standard-CI schlicht fehlt — nicht
   einmal ein „unerreichbarer Server"-Fehlerpfad wie bei FTP/SFTP
   (ADR-0034 Punkt 5) ist hier testbar. `FEATURES.md` muss diese Lücke
   explizit benennen, nicht verschweigen.
6. **KI-Entrauschung/KI-Hochskalierung** verwenden dieselbe
   Ehrlichkeitslinie wie ADR-0033: klassische, deterministische
   Algorithmen (kantenerhaltender Bilateral-Filter statt echter
   neuronaler Entrauschung, kantengerichtete Interpolation statt echter
   Modell-Superresolution) statt einer vorgetäuschten Modellinferenz —
   das ungelöste ONNX-Beschaffungsproblem aus ADR-0033 besteht
   unverändert fort. Die UI-Beschriftung darf an diesen zwei Stellen
   nicht „KI"/„AI" implizieren, wo keine Modellinferenz läuft.
7. **Personenansicht (Gesichtserkennung)** nutzt dieselbe Art
   klassischer Heuristik wie die Phase-7-KI-Masken (Hautton-/
   Kontur-Erkennung im YCbCr-Raum, grobe Ähnlichkeitsgruppierung) statt
   echter Gesichts-Embeddings — dieselbe ONNX-Beschaffungslage.
8. **Adobe-`.xmp`/`.lrtemplate`-Interop** bekommt ein eigenes,
   dokumentiertes Mapping `EdlV3` ↔ Adobe-Parameternamen; welche Felder
   verlustfrei und welche nur best-effort übersetzbar sind (Adobes
   proprietäre Kurven-/Masken-Repräsentation ist nicht vollständig
   offen dokumentiert), wird bei Umsetzung explizit in `FEATURES.md`
   festgehalten, nicht stillschweigend als „Fertig" verbucht.

**Explizit ausgenommen:** Maskentyp Tiefenbereich (`FEATURES.md`,
weiterhin „Später zurückgestellt", siehe ADR-0032 Punkt 3) — kein
Tiefendaten-Zulieferer existiert irgendwo im Projekt, jede Umsetzung
wäre erfunden statt real. Bleibt ohne Phasenzuordnung, bis eine
Tiefendatenquelle auftaucht.

**Konsequenzen:** `PLAN.md` bekommt einen neuen Abschnitt „Aktuelle
Phase: Phase 9" mit zwölf Bauschritten in der oben begründeten
Reihenfolge (Bibliothek zuerst als Fundament für Stacking, dann
Entwickeln-Erweiterungen nach Risiko sortiert, dann die sechs
SPEC-§5/§3.6-„Fortgeschrittenes"-Themen). `FEATURES.md`s Phase-9-Zeilen
bleiben bis zur jeweiligen Umsetzung „Nicht begonnen" — keine Korrektur
nötig, beide Recherchen vor dieser ADR bestätigen, dass sie bereits
vollständig und korrekt getaggt sind. Jeder Bauschritt wird einzeln
committet und gepusht, mit schlanken statt erschöpfenden Tests je
Schritt (Projektkonvention seit Phase 8) — keine wiederholten vollen
Testsuiten-Läufe zwischen den Schritten.

## ADR-0036: Phase-9-Abnahme (Schritt 12) — Stapelverarbeitungs-Konsole war in ADR-0035s Scope-Aufzählung, bekam aber nie einen Bauschritt; bleibt explizit zurückgestellt statt nachträglich überstürzt angeflanscht

**Status:** Angenommen
**Kontext:** Beim Abnahme-Durchgang (`PLAN.md` Schritt 12) fiel auf: Die
„Stapelverarbeitungs-Konsole" (`SPEC.md` §3.6: „Regeln auf Tausende
Bilder anwenden, mit Vorschau der betroffenen Menge, Trockenlauf,
Rückgängig-Machen der gesamten Aktion") steht in ADR-0035s Aufzählung
der sechs SPEC-§3.6-„Zusätzliche Module" (zusammen mit Astro-Stacking,
Vergleichs-Grid, Zeitleisten-Ansicht, Verlaufs-Vergleich,
Kollaborationsmodus) — bekam aber, anders als die anderen fünf, **nie**
einen eigenen Bauschritt in `PLAN.md`s Schritt-für-Schritt-Plan (Schritt
1–11). Eine Lücke im Schritt-0-Zuschnitt selbst, kein Ausrutscher bei
der Umsetzung eines vorhandenen Schritts — dieselbe Kategorie Fehler wie
die in Schritt 11 nachgetragenen `THIRD_PARTY.md`-Zeilen, nur auf der
Planungs- statt der Dokumentationsebene.

**Abwägung:** Der Nutzer hat für diese Runde ausdrücklich verlangt, die
Phase vollständig abzuschließen, nicht abzubrechen. Trotzdem wird diese
Lücke hier **nicht** durch eine nachträglich in Schritt 12
hineingequetschte Implementierung geschlossen: Ein ehrliches
„Rückgängig-Machen der gesamten Aktion" für eine Stapelverarbeitungs-
Konsole bräuchte einen neuen, feldübergreifenden Batch-Operationen-
Log (nicht nur `edit_history` je Foto, sondern eine Journalstruktur über
beliebige Katalogmutationen — Bewertung, Schlagworte, Metadaten,
Löschen — mit Gruppierung „gehört zu Aktion X" und einem echten
Undo-Pfad dafür). Das ist derselbe Umfang wie ein eigener Bauschritt,
kein Dokumentations-Anhängsel — eine überstürzte Version ohne echtes
Batch-Undo (z. B. nur eine Vorschau-Anzeige ohne Rückgängig-Funktion)
würde eine Fähigkeit vortäuschen, die nicht vollständig da ist, exakt
das, was `PLAN.md`/`DECISIONS.md` an anderer Stelle konsequent vermeidet
(siehe z. B. ADR-0033, ADR-0035 Punkt 6).

**Entscheidung:** Stapelverarbeitungs-Konsole bleibt in `FEATURES.md`
explizit „Zurückgestellt" (nicht „Nicht begonnen" — das würde die
fehlende Schrittzuordnung verschleiern), mit Verweis auf diese ADR.
Kein neuer Bauschritt in dieser Phase; bei Bedarf eigener Schritt in
einer späteren Phase, sobald ein Batch-Operationen-Log tatsächlich
gebraucht wird (z. B. wenn ein weiteres Feature eine Undo-fähige
Gruppierung mehrerer Katalogmutationen voraussetzt).

**Konsequenzen:** Alle anderen 37 der 38 in ADR-0035 aufgezählten Punkte
sind mit Schritt 11 vollständig umgesetzt (bzw. mit dokumentierter
bewusster Vereinfachung, siehe ADR-0035 Punkte 1–8) — Phase 9 gilt mit
dieser einen, hier explizit benannten und begründeten Ausnahme als
abgeschlossen.

## ADR-0037: Phase-10-Scope — drei Phase-3/5-UI-Restposten reingezogen, Testdisziplin für diese Phase nutzerangeordnet gelockert, ehrliche Grenzen bei Installer-Signierung

**Status:** Angenommen
**Kontext:** Der Nutzer hat Phase 10 („Politur", `SPEC.md` §5: Performance-
Profiling gegen die Ziele aus §2.4, Barrierefreiheit, Lokalisierung DE/EN,
Onboarding, Installer und Signierung für alle drei Plattformen) angewiesen,
mit zwei ausdrücklichen Vorgaben: (1) erst ein vollständiger Plan, dann alle
Schritte ohne Zwischenstopp — dieselbe Disziplin wie Phase 9; (2) Fokus vor
allem auf die UI, nicht auf weitere Tests, mit nur einer vollen Testsuite
am Ende statt der sonst pro Schritt üblichen.

**Entscheidung 1 — Scope-Erweiterung um drei UI-Restposten:** Drei
`FEATURES.md`-Zeilen standen noch auf Phase 3/5, wurden dort aber nie
angefasst: rechte Werkzeug-Palette/Modul-Umschalter oben (Phase 3),
ein-/ausklappbare/breitenziehbare Paletten mit Arbeitsbereich-Preset
(Phase 3), vollständige Befehlspalette (Phase 5, bisher nur
Ordner+ein Befehl). Alle drei sind reine UI-Arbeit und passen exakt zum
Auftrag „vor allem UI" — sie werden analog zu ADR-0032 (das
Bibliotheks-Backlog von Phase 6 nach Phase 9 verschob) in Phase 10 gezogen,
statt sie ein drittes Mal auf eine noch spätere Phase zu vertagen.

**Entscheidung 2 — Testdisziplin gelockert, nur für diese Phase:**
`PLAN.md` §6 verlangt normalerweise „Jedes Rust-Modul mit Unit-Tests …
E2E-Test pro Modul" pro Schritt. Auf ausdrücklichen Nutzerwunsch schreibt
Phase 10 **pro Schritt keine neue Testdatei** — nur `cargo build`/`tsc -b`
als Kompilier-Kontrolle. Die volle Suite (`cargo fmt/clippy/test`,
`tsc -b`, `vitest run`, volle Playwright-Suite) läuft einmalig am Ende
(Schritt 12) gegen den gesamten in dieser Phase entstandenen Stand. Das ist
eine bewusste, befristete Ausnahme von der sonst verbindlichen Regel — sie
gilt nicht rückwirkend für Phase 1–9 und nicht automatisch für Phase 11+,
falls es eine gibt.

**Entscheidung 3 — Installer/Signierung ehrlich begrenzt:** Diese
Linux-Sandbox besitzt keine echten Code-Signing-Zertifikate (Apple
Developer ID + Notarisierungs-Zugangsdaten, Windows-Codesigning-Zertifikat)
und kann sie nicht beschaffen — dieselbe Beschaffungslage wie
`libgphoto2` (ADR-0035 Punkt 5) oder die fehlende ONNX-Runtime
(ADR-0033). Schritt 11 baut die strukturelle Infrastruktur (Tauri-
Bundler-Konfiguration, `@tauri-apps/cli`, ein neuer CI-`release`-Job auf
dem bestehenden 3-OS-Matrix, Signierungsfelder aus optionalen
GitHub-Secrets gespeist, übersprungen statt fehlschlagend, wenn sie
fehlen) — produziert aber in dieser Umgebung **unsignierte** Installer.
Echtes Signieren bleibt dem Nutzer vorbehalten, sobald eigene
Zertifikate/Secrets hinterlegt sind. Cross-Plattform-Bundling
(macOS-/Windows-Installer von Linux aus) wird nicht lokal versucht — die
Verifikation läuft ausschließlich über die drei echten nativen CI-Runner.

**Konsequenzen:** `FEATURES.md`s Phase-10-Zeilen wachsen um die drei
umgetaggten Punkte. Performance-Profiling (Schritt 10) liefert eine
dokumentierte Einschätzung gegen die in dieser Sandbox tatsächlich
messbaren Teilziele statt spekulativer Umbauten ohne Befund — dieselbe
Grenze wie die fehlenden Golden-Image-RAW-Tests (ADR-0007). Lokalisierung
(Schritt 8) deckt systematisch alle Frontend-Komponenten ab, aber ohne
einen dafür geschriebenen Coverage-Test ist einzelne übersehene Strings
nicht mit Sicherheit ausgeschlossen — als offener, dokumentierter Rest
statt stillschweigend behauptet.

## ADR-0038: Phase 11 — Nachträge zu allen zurückgestellten Punkten aus Phase 1–10; vier neu verfügbare Crates real geprüft (drei brauchbar, eine als Fassade entlarvt), `libgphoto2` ist entgegen ADR-0035 tatsächlich per `apt` verfügbar

**Status:** Angenommen
**Kontext:** Der Nutzer hat angewiesen, alle bisher zurückgestellten Punkte
aus Phase 1–10 aufzuarbeiten — mit der ausdrücklichen Vorgabe, für jeden
Punkt neu zu prüfen, ob er inzwischen technisch machbar ist, statt ihn
unangetastet zu lassen (dieselbe Disziplin, mit der dieses Projekt schon
mehrfach Crate-Landschaften neu geprüft statt Annahmen fortgeschrieben
hat). `FEATURES.md` listete zu Beginn dieser Runde neun offene
Checkbox-Zeilen; dazu kommen vier in der Phase-10-Abnahme selbst benannte
Lücken.

**Dokumentationsfehler gefunden und korrigiert:** `FEATURES.md` Zeile 239
führte „Adobe `.xmp`/`.lrtemplate`-Import/Export" noch als „Nicht begonnen",
obwohl `apx_export::xmp` (`crates/apx-export/src/xmp.rs`, gebaut in Phase 9
Schritt 2) den `.xmp`-Teil (Adobe `crs:`-Entwickeln-Einstellungen: Basic+HSL,
echt bidirektional per `generate_xmp`/`parse_xmp_develop_settings`, über
`import_xmp_develop_settings`/`import_xmp_sidecar_from_file` bis ins
Frontend verdrahtet) bereits vollständig abdeckt. Nur der separate
`.lrtemplate`-Teil (Lightrooms eigenes, nicht offiziell dokumentiertes
Vorlagenformat) war tatsächlich noch offen — die Zeile wurde entsprechend
präzisiert statt pauschal als „Nicht begonnen" stehen zu lassen.

**Crate-Spikes** (`cargo add --dry-run` + realer Testbau in einem
Wegwerf-Projekt, kein Produktivcode betroffen):
1. **`gamut-dng`** (1.0.0, MIT/Apache-2.0) — kompiliert sauber, reines Rust,
   `DngEncoder`/`DngDecoder` mit vollständiger Builder-API. Zum Zeitpunkt
   von ADR-0034 (Phase 8) gab es keine schreibfähige DNG-Crate; diese
   existiert jetzt. **Brauchbar** — siehe Schritt 1.
2. **`gamut-jxl`** (0.4.0, MIT/Apache-2.0, Encode-Feature) — kompiliert,
   aber nicht reines Rust: `gamut-jxl-sys` vendort und baut die
   Referenz-C-Bibliothek `libjxl` (Lizenz BSD-3-Clause) selbst über
   `cmake`/`cc` zur Bauzeit (kein System-`apt`-Paket nötig, aber ein
   spürbar schwererer Build, ca. 1,5 Minuten allein für diese eine
   Abhängigkeit in der Spike-Messung). **Brauchbar, aber teurer** — siehe
   Schritt 2.
3. **`ag-psd`** (0.2.0, MIT) — kompiliert sauber, reines Rust, „from-scratch
   Rust port" der TypeScript-`ag-psd`-Bibliothek, `write_psd(&Psd,
   &WriteOptions) -> Vec<u8>` als klare Top-Level-API. **Brauchbar** —
   siehe Schritt 2.
4. **`heif`** (0.1.0) — laut Beschreibung „A HEIF file decoder and encoder
   written from scratch", tatsächlich aber ein reserviertes Fassaden-Paket:
   `src/lib.rs` ist 14 Zeilen lang und enthält ausschließlich `cargo new`s
   Standard-Vorlage (`pub fn add(left, right) -> u64`). **Nicht brauchbar**
   — bestätigt, warum dieses Projekt Beschreibungen nie ungeprüft
   übernimmt. `heif-rs` (26.7.0, „statically-linked libheif") wurde
   testweise ebenfalls hinzugefügt: zieht über 190 Pakete nach, darunter
   `bindgen` (braucht `libclang`), einen kompletten `image`-Crate-Formats-
   Rattenschwanz (`gif`/`exr`/`zip`/…) und `ureq` (HTTP-Client — unklar,
   wofür ein Bild-Codec Netzwerkzugriff bräuchte) — derselbe Beschaffungs-
   Risikograd wie ONNX Runtime/`opencv` an anderer Stelle im Projekt, bei
   nur 6 GB freiem Speicher in dieser Sandbox nicht vertretbar zu bauen.
   **HEIF-Export bleibt zurückgestellt**, wie schon in ADR-0034.
5. **`libgphoto2-dev`** ist entgegen der in ADR-0035 getroffenen Annahme
   („auf keinem der drei CI-Runner installiert") tatsächlich per `apt-get`
   installierbar (`apt-cache policy` zeigt Version 2.5.31-2.1ubuntu1.1
   verfügbar) — die ADR-0035-Aussage war für diese Sandbox richtig
   *behauptet*, aber nie tatsächlich mit `apt-cache policy` verifiziert
   worden. Siehe Schritt 10: das `tethering`-Feature kann jetzt real
   kompiliert und sein Verbindungs-Fehlerpfad ohne physische Kamera echt
   getestet werden.

**Entscheidung:** Phase 11 bündelt alle neun `FEATURES.md`-Zeilen plus die
vier Phase-10-Lücken plus die eine gefundene Dokumentationskorrektur in
13 Bauschritten (0–12), mit der normalen, vollen Testdisziplin aus
`PLAN.md` §6 (die Lockerung aus ADR-0037 galt ausdrücklich nur für
Phase 10). Für PSD/JPEG-XL/DNG werden die oben real geprüften Crates
verwendet; für HEIF, echte Zertifikats-Signierung, echte GPU-Performance
und echte neuronale Modellinferenz bleiben die bereits in ADR-0033/-0034/
diesem Plan dokumentierten Grenzen unverändert bestehen — kein
Software-Trick ersetzt fehlende Hardware, ein fehlendes Zertifikat oder
ein nicht beschaffbares Modell.

**Nachtrag nach Schritt 4 — Testdisziplin nutzerseitig erneut gelockert:**
der Nutzer hat nach Abschluss von Schritt 3 angewiesen, ab Schritt 4 nur
noch maximal einen Test pro Schritt zu schreiben und die vollständige
Verifikationskette (`cargo fmt/clippy/test --workspace`, `tsc -b`, volle
Vitest-/Playwright-Suite) nicht mehr nach jedem einzelnen Schritt,
sondern erst einmalig am Ende in Schritt 12 laufen zu lassen — dieselbe
Lockerung wie in ADR-0037 für Phase 10, nur diesmal ausdrücklich auch für
Phase 11 (Schritte 4–11) angeordnet, statt wie oben ursprünglich
festgehalten strikt auf Phase 10 begrenzt. Schritte 0–3 liefen noch mit
der vollen, oben beschriebenen Disziplin; ab Schritt 4 gilt: pro Schritt
ein gezielter Test (Rust-Unit- oder Playwright-Test, je nachdem, welcher
den Kern der Änderung am direktesten abdeckt) plus ein gezielter
Kompilier-/Typprüf-Lauf des geänderten Codes (kein vollständiger
Testlauf) vor Commit+Push.

**Nachtrag nach Schritt 7 — Plan-Abweichung `apx_ai::depth_estimate`:**
der Plan sah die Laplace-Varianz-Schärfeheuristik für den neuen
`BlurDepthApprox`-Maskentyp in `apx_ai::depth_estimate::
relative_sharpness_map` vor. Tatsächlich implementiert wurde sie
stattdessen direkt in `apx-pipeline::stages::masks` (wie
`color_range_alpha`/`luminance_range_alpha`) — `apx-ai` hängt von
`apx-pipeline` ab (siehe dessen `Cargo.toml`), eine Abhängigkeit in die
umgekehrte Richtung wäre ein Zyklus gewesen. `MaskGeometry::
BlurDepthApprox { threshold: f32 }` wird wie `ColorRange`/
`LuminanceRange` live pro Render berechnet, nicht als vorab in `apx-ai`
gebackene Alpha-Bitmap (anders als `AiGenerated`) — deshalb passt die
Eigenständigkeit in `apx-pipeline` auch inhaltlich besser als ein
Umweg über `apx-ai`.

**Nachtrag nach Schritt 11 — Phase-10-Nachträge, mit einer weiteren
ehrlichen Grenze bei der Installer-Signierung:**
- **Lokalisierung**: die 13 in der Phase-10-Abnahme namentlich als
  unübersetzt benannten Dialog-Komponenten (Export/Druck/Diashow/Buch/
  Web/Vorlagen/Organisieren/Stacking/Skript & Plugins/Kollaboration/
  Tethering/Metadaten-Editor/Statistik) sind jetzt vollständig über
  `lib/i18n.ts`s `t()`-Muster übersetzt (de.ts/en.ts). `SlideshowPlayer.tsx`
  (die separate Vollbild-Wiedergabekomponente, die `SlideshowDialog.tsx`
  bei „Abspielen" öffnet) bleibt bewusst außerhalb dieses Schritts —
  ehrlich als offene Ausbaustufe stehen gelassen statt stillschweigend
  mitgezählt, ebenso die von `MetadataDialog.tsx` wiederverwendeten
  `PRESET_CONDITION_FIELD_OPTIONS`/`PRESET_CONDITION_OPERATOR_OPTIONS`-
  Labels aus `lib/presets.ts` (gemeinsam mit `SavePresetDialog.tsx`
  genutzt, eigener Schritt nötig).
- **`PaletteFrame`-Ausrollung**: `DevelopPanel.tsx`/`MasksPanel.tsx` sind
  jetzt in `PaletteFrame` gewrappt (Ziehen/Einklappen wie die übrigen vier
  Paletten). `MasksPanel.tsx`s `id="stage-masks"`-Sprunganker (von
  `DevelopPanel.tsx`s Node-Editor „Öffnen"-Link genutzt) wandert dabei auf
  die `<h2>`-Überschrift innerhalb des neuen `PaletteFrame`, weil dieser
  selbst kein durchgereichtes `id`-Attribut auf sein `<aside>` anbietet —
  funktional identisch (`scrollIntoView` scrollt denselben Container).
  Direkt danach volle `develop-flow.spec.ts`/`masks-flow.spec.ts`-Regression
  gefahren (40 Tests grün) — dieselbe Vorsicht, die Phase 10 Schritt 3
  zurückgestellt hatte, jetzt mit Testnetz.
- **Umbelegbare lokale Tastenkürzel**: `Viewer.tsx`s Zoom-Zifferntasten
  (neu: `zoom-fit`/`zoom-100`) und `DevelopPanel.tsx`s eigener Ctrl/Cmd+Z-
  Handler laufen jetzt über `lib/keybindings.ts`s `matchesBinding` statt
  fest verdrahteter `event.key`-Vergleiche. Der Entwickeln-Panel-Handler
  nutzt bewusst dieselben `"undo"`/`"redo"`-IDs wie `App.tsx`s Bibliotheks-
  Metadaten-Undo (statt eigener neuer IDs), weil sich beide Kontexte
  gegenseitig ausschließen (`App.tsx` reicht Ctrl/Cmd+Z nur weiter, wenn
  das Entwickeln-Panel geschlossen ist) — ein Nutzer, der „Rückgängig"
  umbelegt, bekommt eine konsistente neue Taste in beiden Kontexten.
  Bewusst weiterhin fest: Kurven-/Masken-Editoren mit
  `role="slider"`-Pfeiltasten-Feinjustierung und die Bewertungs-
  Zifferntasten 0–5 (parametrisierte Ziffernreihe, keine einzelne feste
  Aktion). Ein neuer Playwright-Test (max. 1 pro Schritt, siehe oben)
  deckt beides ab: Ctrl+Z/Ctrl+Shift+Z committen jetzt tatsächlich über
  den Tastatur-Pfad im Entwickeln-Panel (die bestehende
  `develop-flow.spec.ts`-Suite deckte bislang nur den Rückgängig-Knopf ab,
  nicht die Taste), und eine Neu-Belegung im Cheatsheet-Overlay steuert
  Ctrl+Z tatsächlich um.
- **Installer-Signierung — Mechanik-Nachweis, weiter eingeschränkt als
  im Plan vorgesehen**: der Plan sah vor, ein selbstsigniertes Test-
  Zertifikat einmalig als echtes GitHub-Actions-Secret zu setzen und
  dadurch `ci.yml`s `release`-Job (Windows-Zertifikatsimport/
  Fingerabdruck-Ermittlung/`--config`-Override) real durchlaufen zu
  lassen. **Diese Sitzung hat kein Werkzeug, um Repository-Secrets zu
  schreiben** (kein Admin-API-Zugriff über die verfügbaren GitHub-Tools),
  und keinen Windows-/macOS-Ausführungskontext (reine Linux-Sandbox) —
  ein echter CI-Lauf des `release`-Jobs war damit nicht möglich, ohne
  einen Menschen um das Setzen des Secrets zu bitten (außerhalb des
  Scopes dieser Sitzung). Stattdessen lokal verifiziert, **nur der
  betriebssystemunabhängige Teil der Mechanik**: ein selbstsigniertes
  Test-Zertifikat wurde per `openssl` erzeugt, zu einem PFX/P12-Bundle
  gepackt (dasselbe Format wie `WINDOWS_CERTIFICATE`/`APPLE_CERTIFICATE`),
  Base64-kodiert und wieder dekodiert (exakt der Schritt, den `ci.yml`s
  `[Convert]::FromBase64String` beim echten Secret durchführt) —
  Byte-für-Byte identisch zum Original bestätigt — und sein SHA1-
  Fingerabdruck ermittelt (dieselbe Kennzahl, die Windows unter
  `$cert.Thumbprint` liefert, nur von `openssl x509 -fingerprint -sha1`
  anders formatiert ausgegeben). Zertifikat/Schlüssel/PFX wurden direkt
  danach gelöscht, nichts committet. **Bleibt offen**: der eigentliche
  `Import-PfxCertificate`-Aufruf in einen Windows-Zertifikatspeicher und
  ein echter `tauri build`-Lauf mit `--config`-Override sind weiterhin
  nie ausgeführt worden — dieselbe Grenze wie in ADR-0037, jetzt nur um
  den zusätzlichen Befund ergänzt, dass auch der Secret-Setzschritt
  selbst außerhalb der Werkzeuge dieser Sitzung liegt.

## ADR-0039: Phase 12 — Lightroom-Lückenschluss; `lensfun`-Crate real geprüft und für Schritt 3 freigegeben

**Status:** Angenommen
**Kontext:** Der Nutzer hat einen ausführlichen, bidirektionalen
Funktionsvergleich von ApertureX gegen Adobe Lightroom Classic/CC in
Auftrag gegeben (veröffentlicht als Artifact „ApertureX vs. Lightroom"),
jede Zeile aus `FEATURES.md` gegen den echten Lightroom-Funktionsumfang
geprüft. Direkt im Anschluss wurde angewiesen, für die dort gefundenen
Lücken einen Plan mit tief recherchierten, echten Lösungen zu schreiben —
mit ausdrücklichem Fokus auf die Frage, ob der Kamera-/Objektiv-Profil-
Import über eine „KI-generierte" Lösung tragen könnte.

**Befund zur KI-Frage:** eine echte Modellinferenz für Verzeichnungs-/
Vignettierungs-Koeffizienten aus einem einzelnen Foto ohne Kalibrierziel
bräuchte trainierte Gewichte — dieselbe seit ADR-0033 dokumentierte Wand
wie jede andere „KI"-Funktion in diesem Projekt. Eine LLM-„Schätzung"
von Objektiv-Koeffizienten ohne echte Kalibrierdatengrundlage wäre reine
Fabrikation und wird hier bewusst **nicht** vorgeschlagen.

**Stattdessen real gefunden — die `lensfun`-Crate** (crates.io, v0.7.0,
reines Rust, LGPL-3.0-or-later, Autor David Veszelovszki, Repo
`github.com/vdavid/lensfun-rs`): ein gegen die C++-Referenzbibliothek
bit-exakt getesteter Port (laut Projekt-README 1.640 A/B-Testfälle,
Abweichung 4,88×10⁻⁴ Pixel) der echten, offenen LensFun-Objektivdatenbank.
`Database::load_bundled()` liefert Tausende real kalibrierte Kamera-/
Objektiv-Kombinationen direkt eingebettet (~574 KB gzip), ohne
Laufzeit-Dateisystemzugriff. Das macht den bisherigen 3-Profile-
Platzhalter (ADR-0028) für Schritt 3 gegenstandslos — **andere
Datenquelle** (offene LensFun-Datenbank statt Adobe-DCP/LCP), **gleiche
Wirkung**, ohne das ungelöste Adobe-Format-Problem selbst anzufassen.

**Spike real durchgeführt** (nicht nur `--dry-run`): `cargo add
lensfun@0.7` in `apx-pipeline`, `cargo build -p apx-pipeline` erfolgreich
(„Finished dev profile … in 2m 01s"), einzige neue Abhängigkeit ist
`roxmltree` (reines Rust, DOM-XML-Parser) — keine C-Bindings, kein
`bindgen`/`libclang`. Ein Test lädt die gebündelte Datenbank, findet
`Canon EOS 5D Mark III` + `Canon EF 16-35mm f/4L IS USM` real in den
Daten und liest deren Verzeichnungskalibrierung bei 16mm aus
(`crates/apx-pipeline/src/lens_profiles.rs`,
`lensfun_bundled_database_has_plausible_distortion_calibration_for_known_lens`).
**Ehrlicher Befund dabei:** `lensfun`s Poly3-`k1` (0,0128 für dieses
Objektiv) folgt einer anderen Vorzeichen-/Skalierungskonvention als
unser bisheriges `generic-wide`-Profil (`distortion_k1 = -0,12`, eigene
Konvention seit ADR-0028) — ein direkter Zahlenvergleich zwischen beiden
Systemen ist nicht aussagekräftig; die echte Umrechnung (inkl.
`Modifier`s Re-Skalierung auf Bildmaße/Cropfaktor) ist Aufgabe von
Schritt 3 Teil A. Der Spike verifiziert nur die Grundvoraussetzung:
die Datenbank liefert für real existierende Objektive nutzbare,
begrenzte Kalibrierwerte.

**Für Objektive außerhalb der Datenbank** (Teil B, Schritt 3): ein
Kalibrier-Assistent, der aus vom Nutzer selbst fotografierten
Schachbrett-Kalibrierbildern per klassischer Zhang-Methode (Ecken-
erkennung, Homographie-Schätzung, nichtlineare Verfeinerung — reine
Optimierung, kein gelerntes Modell) ein Profil berechnet. Wird im Dialog
und in `FEATURES.md` ehrlich als „aus eigenen Kalibrierfotos berechnet"
beschriftet, nicht als „KI-generiert".

**Entscheidung:** Phase 12 bündelt alle im Lightroom-Vergleichs-Artifact
gefundenen, tatsächlich schließbaren Lücken in neun Bauschritten (0–8):
Live-Masken-Overlay, Radialverlauf-Ellipse + Auto-Mask, echte LensFun-
Datenbank + Kalibrier-Assistent, voller EXIF/IPTC-Editor, Mehrfachziel-
Export, freies ICC-Profil beim Soft-Proof, beobachtete Ordner/Auto-
Import. **Bewusst ausgeklammert** (echte Design-Entscheidung oder bereits
dokumentiertes Beschaffungsproblem ohne neue Datenlage, siehe Vergleichs-
Artifact): Cloud-Synchronisation/mobile Begleit-App, Mehrfach-Katalog,
Publish Services, Print-on-Demand-Bestellintegration, Adobe-kompatibles
Plugin-SDK, generative KI-Bildbearbeitung, HEIF-Export (ADR-0038 bereits
geprüft, keine neue Datenlage). Testdisziplin wie vom Nutzer für den Rest
von Phase 11 angeordnet fortgeführt: ein gezielter Test pro Schritt, volle
Suite einmalig in Schritt 8.

## ADR-0039-Nachtrag: Schritt 3 real umgesetzt — echte Ecken-Rückrechnung statt Koeffizienten-Übernahme, vereinfachter Kalibrier-Assistent statt vollem Zhang-Verfahren

**Status:** Angenommen
**Kontext:** Schritt 3 Teil A/B wurden umgesetzt. Zwei Stellen weichen
ehrlich vom ursprünglichen ADR-0039-Text ab — beide Male, weil die
Recherche beim Bauen mehr Klarheit brachte als beim Planen.

**Teil A — Befund, der die geplante Schema-Migration gegenstandslos
machte:** `radius_x`/`radius_y`/`angle_degrees` (Schritt 2) UND die
Grundannahme für Schritt 3 wurden geprüft, bevor Code geschrieben wurde
— dabei stellte sich heraus, dass `LensCorrectionAdjustment`s einzige
wirkliche Lücke die *Datenquelle* für `distortion_k1`/`vignette_amount`/
`ca_red_cyan`/`ca_blue_yellow` war, nicht das Feldschema selbst. Die
eigentliche Herausforderung: `lensfun`s eigene Modelle (Poly3/Poly5/
PTLens für Verzeichnung, mehrgliedrige TCA-/Vignettierungs-Polynome)
sind reichhaltiger als unser Ein-Wert-r²-Modell — ein LensFun-
Koeffizient lässt sich nicht 1:1 übernehmen, selbst mit korrekter
Einheiten-Umrechnung, weil die Kurvenform selbst eine andere ist (exakt
die Lücke, die der Schritt-0-Spike als „Aufgabe von Schritt 3" benannt
hatte). Gelöst über eine an LensFuns *eigener* `Modifier`-Pixel-
mathematik verankerte Rückrechnung: die reale, mehrparametrige Korrektur
wird an der Ecke eines repräsentativen 3:2-Referenzbilds ausgewertet
(`apply_geometry_distortion`/`apply_color_modification_f32`/
`apply_subpixel_distortion` — dieselben Funktionen, die ein Foto real
korrigieren würden), und daraus ein einzelner Koeffizient gesucht, der
in unserem Modell an derselben Stelle dieselbe Wirkung erzeugt (siehe
`crates/apx-pipeline/src/lens_profiles.rs`s
`derive_lens_correction_values`). Eine echte, nachvollziehbare Näherung
— keine geratene Zahl —, mit der ehrlichen Grenze, dass sie nur an der
Bildecke exakt stimmt.

**Teil B — bewusst kein Zhang-Verfahren:** der ursprüngliche Plantext
nannte „Eckenerkennung per Harris-artigem Detektor + Homografie-
Schätzung + nichtlineare Verfeinerung". Eine robuste automatische
Schachbrett-Eckenerkennung plus volle Mehrparameter-Kamerakalibrierung
ist ein eigenständiges, fehleranfälliges Computer-Vision-Projekt für
sich — für unser Ein-Wert-Verzeichnungsmodell überdimensioniert und in
diesem Umfang nicht seriös umsetzbar. Stattdessen implementiert
`apx-ai::lens_calibration` eine bewusst schmalere, aber ebenso reale
Methode: der Nutzer markiert selbst mehrere Punkte entlang einer in der
Realität geraden Linie (direkt auf einer Bildvorschau im neuen Dialog
„Objektiv kalibrieren", `LensCalibrationDialog.tsx`), und
`calibrate_distortion_k1` sucht per Rasterverfeinerung den einen
Verzeichnungskoeffizienten, der alle markierten Linien nach der
Entzerrung gemeinsam am geradesten macht (totale Kleinste-Quadrate für
die Geradheit, klassische 1-D-Optimierung für die Suche — kein
gelerntes Modell). Ein Test mit synthetisch verzeichneten Linien
bestätigt, dass ein bekannter Koeffizient wiedergefunden wird
(Abweichung < 0,01). Ergebnis lebt direkt im EDL
(`LensCorrectionAdjustment::custom_distortion_k1`, additiv per
`#[serde(default)]`) statt in einer neuen Profildatenbank/-datei —
Wiederverwendung auf andere Fotos über die seit Phase 5 vorhandene
Einstellungen-kopieren-Funktion, kein neuer Persistenzmechanismus nötig.
Bewusst nur Verzeichnung (Vignette/CA bräuchten andere Messungen –
Helligkeits- bzw. Kanal-Registrierung statt Geradheit – nicht Teil
dieses Umfangs).

**Entscheidung:** Beide Abweichungen sind Umfangs-Präzisierungen, keine
Kürzungen am eigentlichen Nutzen — Teil A liefert weiterhin eine echte,
Datenbank-gestützte Objektiv-Zuordnung (jetzt sogar automatisch beim
Fotowechsel), Teil B weiterhin eine echte, aus eigenen Fotos berechnete
Kalibrierung für Objektive außerhalb der Datenbank. `FEATURES.md` und
`PLAN.md` sind entsprechend präzisiert.

## ADR-0039-Nachtrag II: Schritt 6 — echter ICC-Soft-Proof über `lcms2::Transform::new_proofing` ersetzt die JS-Näherung

**Status:** Angenommen
**Kontext:** Der Soft-Proof im Entwickeln-Panel (Phase 6 Schritt 10,
ADR-0032 Punkt 6) war bis hierhin eine rein clientseitige Sättigungs-
Näherung mit drei erfundenen "simulierten" Zielen (`srgb`/`print_sim`/
`grayscale_sim`) — ADR-0032 selbst nannte das explizit als bewusste
Vereinfachung, mit "kein echtes 3D-Gamut-Mapping" als offen benannte
Lücke. `apx_export::icc` bindet `lcms2` bereits seit Phase 8 Schritt 2
für den Export ein (`convert_from_srgb`, inklusive `IccTarget::
CustomFile` für eine vom Nutzer gewählte `.icc`-Datei) — dieselbe
Bibliothek unterstützt über `Transform::new_proofing` (kombiniert mit
den `SOFT_PROOFING`/`GAMUT_CHECK`-Flags und `cmsSetAlarmCodes` für die
Farbumfangswarnung) echtes, standardbasiertes Soft-Proofing, exakt die
Funktion, die auch Lightroom/Photoshop intern nutzen.

**Umsetzung:** neue Funktion `apx_export::icc::soft_proof_rgba8`
(`Transform::new_proofing(sRGB-Anzeigeprofil, …, Zielprofil, Intent,
Intent, Flags)`) für die vier gebündelten Standardprofile UND eine
beliebige `.icc`-Datei — dieselbe `IccTarget`-Wiederverwendung wie beim
Export. Statt eines neuen Tauri-Commands (der Puffer wäre pro
Regler-Tick zu groß für JSON-serialisierte IPC, siehe die bestehende
Begründung in `crates/apx-app/src/protocol/mod.rs`s Moduldoku, "ohne den
Umweg über Base64-kodierte Tauri-Commands") läuft der Soft-Proof über
ein zusätzliches `<soft_proof>`-Segment derselben `develop/...`-Route,
die auch die normale Vorschau liefert (`none` oder base64url-kodiertes
JSON, siehe `crates/apx-app/src/protocol/route.rs`s Moduldoku) — der
Server liefert bei aktivem Soft-Proof direkt den fertig transformierten
Puffer, kein zweiter Nachbearbeitungsschritt im Frontend für Farbe/Gamut.

**Bewusst erhaltene, kleinere Vereinfachung:** die Papierweiß-Simulation
hat in `lcms2` keine eingebaute Entsprechung (in echten
Bildbearbeitungsprogrammen meist eine separate, dem ICC-Proofing
nachgeschaltete Tonwertkompression) und bleibt daher eine kleine
clientseitige Nachbearbeitung (`lib/softProof.ts::applyPaperWhite`) —
anders als vorher betrifft sie aber nur noch den Tonwertbereich, nicht
mehr Farbe/Sättigung/Gamut. Aus demselben Grund bleibt `developFrame`
selbst (für Farbaufnehmer/TAT/Clipping-Overlay) immer der unveränderte,
nicht soft-proofte Puffer — der Viewer holt bei aktivem Soft-Proof eine
zweite, separate Antwort derselben Route nur für den tatsächlich
gezeichneten Canvas-Inhalt.

**Entscheidung:** `FEATURES.md`/`PLAN.md` sind entsprechend aktualisiert
— Soft-Proof ist ab hier echtes ICC-Farbmanagement, keine Näherung mehr.

## ADR-0039-Nachtrag III: Schritt 7 — Beobachteter Ordner / Auto-Import genau wie geplant umgesetzt

**Status:** Angenommen
**Kontext:** Schritt 7 (Beobachteter Ordner) folgt exakt dem im
Ursprungsplan skizzierten Weg, ohne Abweichung — festgehalten hier nur der
Vollständigkeit halber, wie bei jedem Schritt dieser Phase.

**Umsetzung:** neuer `WatchedFolderSettings`-Block in `apx_core::settings`
(Pfad, an/aus, Poll-Intervall in Sekunden, Default aus) neben den
bestehenden `UiSettings`/`AiSettings`; ein neuer Hintergrund-Task
`watched_folder_worker` in `apx-app/src/main.rs`, nach demselben
Abfragen-statt-Weck-Benachrichtigung-Muster wie der bereits bestehende
`export_queue_worker`. Bei jedem Durchlauf werden die Einstellungen frisch
von der Platte gelesen (ein Umschalten in den Einstellungen wirkt ohne
Neustart) und, falls aktiviert und der Ordner existiert, derselbe
`import::run_with_mode`-Pfad im Modus `AddInPlace` angestoßen wie bei
einem manuellen Import — geteilt über dieselbe `active_import`-Sperre, die
schon einen doppelten manuellen Import verhindert, damit sich ein
automatischer und ein manueller Import nie überschneiden. Kein natives
Datei-System-Watcher-Crate nötig: `run_with_mode` überspringt bereits
katalogisierte Dateien von selbst (`SingleFileOutcome::Unchanged`), ein
wiederholter Lauf über denselben Ordner ist also von sich aus billig und
idempotent — kein eigener "bereits gesehen"-Zustand nötig.

**Entscheidung:** `FEATURES.md`/`PLAN.md` sind entsprechend aktualisiert.

## ADR-0040: Phase 13 — echte ONNX-Laufzeit jetzt real verfügbar (korrigiert ADR-0033 Punkt 1); KI-Ausfüllen, Direktimport, DCP-Profile, klassische CV-Lücken

**Status:** Angenommen
**Kontext:** Aus der aktualisierten Lightroom-Lückenliste (Vergleichs-
Artifact nach Phase 12) hat der Nutzer sechs Punkte ausgewählt:
generatives/bearbeitendes KI-Ausfüllen, Import direkt von Speicherkarte/
Kamera, Adobe-DCP-Farbprofil-Import (fest zugesagt); Perspektive/Upright-
Kantenerkennung und Panorama-Homografie-Stitching (kostenlos geplant);
mehrere Kataloge, ein echter UND/ODER-Regelbauer, echte Personen-
Wiedererkennung ("vielleicht").

**Korrektur an ADR-0033 Punkt 1:** ADR-0033 begründete den Verzicht auf
echte ONNX-Runtime-Modellinferenz u. a. damit, es gebe "keinen
bestätigten Zugriffsweg auf vorkompilierte ONNX-Runtime-Binaries" und
"keinen legitimen Weg, ein trainiertes ... Modell zu beschaffen und
mitzuliefern". Diese Sitzung hat beides real geprüft und beides ist
inzwischen falsch:

1. **Die Laufzeit ist real verfügbar** — per `cargo add --dry-run`
   gegen die echte crates.io-Registry bestätigt: `ort` (2.0.0-rc.13,
   ONNX-Runtime-Bindings) und `tract-onnx` (0.23.6, reines Rust, keine
   C++-Abhängigkeit) sind beide echte, gepflegte Crates.
2. **Für mindestens ein Modell ist auch die Gewichts-Beschaffung
   real gelöst:** LaMa (Large Mask Inpainting, `advimman/lama`,
   Apache-2.0-Code) hat ein echtes, als ONNX exportiertes Modell
   öffentlich auf Hugging Face (`Carve/LaMa-ONNX`, `lama_fp32.onnx`,
   208 MB, Apache-2.0, trainiert auf Places2/CC-BY-4.0) — real
   herunterladbar, lizenzgeklärt, gegen einen veröffentlichten Hash
   verifizierbar. Kein fabriziertes Modell, keine Behauptung ohne
   Beleg — genau die Art Fund, die ADR-0033 damals nicht hatte.

Für dieselbe klassische CV, aber ohne jedes Modellgewicht (Perspektive/
Upright, Panorama-Stitching), sind ebenfalls real verfügbare, reine
Rust-Crates gefunden: `imageproc` (Kantenerkennung/Hough-Transformation),
sowie das `rust-cv`-Ökosystem `akaze`/`arrsac`/`sample-consensus`/
`homography`/`eight-point`/`lambda-twist`/`cv-core`/`space`
(Merkmalserkennung, robuste Homografie-Schätzung) — kein OpenCV nötig.
Für Speicherkarten-Erkennung: `sysinfo` (0.38.4, plattformübergreifende
Wechseldatenträger-Liste).

**Für Gesichts-Wiedererkennung bleibt eine echte Lücke offen:** die
Laufzeit ist dieselbe wie bei LaMa, aber ein gutes vortrainiertes
Gesichts-Embedding-Modell mit wirklich permissiver Lizenz (nicht nur
"Forschung, nicht kommerziell", wie viele InsightFace/ArcFace-
Veröffentlichungen) ist bei dieser Recherche noch nicht bestätigt —
Phase 13 Schritt 8 behandelt das als eigene, ergebnisoffene Prüfung
statt es stillschweigend anzunehmen.

**Entscheidung:** ADR-0033 Punkt 1 gilt als durch diesen Nachtrag
korrigiert (nicht rückwirkend umgeschrieben — der ursprüngliche Text
bleibt stehen, war zum damaligen Rechercheergebnis ehrlich). Phase 13
setzt echte ONNX-Inferenz für das KI-Ausfüllen ein (Schritt 1) und
klassische, gewichtsfreie CV für Perspektive/Panorama (Schritte 4/5).
Vollständiger Schritt-für-Schritt-Plan in `PLAN.md`.

## ADR-0040-Nachtrag: Schritt 3 — echter Adobe-DCP-Import; `ColorMatrix`
geparst, aber bewusst nicht angewendet; `ProfileHueSatMap`/
`ProfileToneCurve` real umgesetzt, direkt von Adobes eigenem
Open-Source-DNG-SDK portiert

**Status:** Angenommen
**Kontext:** `stages::calibration`s bisherige `CAMERA_PROFILES`-Handliste
(sechs feste Namen, je nur ein globaler Sättigungs-/Kontrast-Bias, siehe
ADR-0028) sollte durch echten Import beliebiger Adobe-`.dcp`-Dateien
ersetzt werden — derselbe Sprung wie Phase 12 Schritt 3 von einer
Objektiv-Handliste zur echten LensFun-Datenbank.

**Ergebnis der Recherche:** `.dcp`-Dateien sind reine TIFF/IFD-Container
ohne Rohbild-Daten. `gamut-dng`s `DngDecoder::decode()` (bereits
Abhängigkeit von `apx-export`) verlangt ein echtes Rohbild und scheitert
deshalb an eigenständigen `.dcp`-Dateien. Die real verfügbare Lösung:
`gamut-ifd` (2.0.1, dieselbe IFD-Lese-Grundlage, die `gamut-dng` selbst
intern nutzt) direkt einbinden und nur die tatsächlich benötigten Tags
lesen — per `cargo add --dry-run` gegen die echte crates.io-Registry
bestätigt. Die Tag-Nummern (`ColorMatrix1` 50721 usw.) sind gegen
`gamut-dng`s eigenes öffentliches `tags`-Modul **und** unabhängig davon
gegen Adobes eigenes, quelloffenes DNG-SDK
(`github.com/aizvorski/dng_sdk`) geprüft.

**Zwei Ebenen, unterschiedlich behandelt:**

1. **`ColorMatrix1`/`ColorMatrix2`/`ForwardMatrix1/2`/
   `CameraCalibration1/2`** (die eigentliche Farbmatrix) — wird geparst
   (`dcp_profile::DcpProfile`), aber **bewusst nicht in die Pipeline
   eingespeist**. Eine echte Kamera→XYZ-Matrixumrechnung ist Aufgabe des
   Rohdaten-Decoders (`apx-raw` hat dafür bereits eine eigene
   `cam_to_srgb`-Matrix, einmalig beim Dekodieren angewendet) — ein
   Matrixwechsel je Kalibrierungs-Regler würde eine erneute
   Rohdaten-Dekodierung bei jedem Bearbeitungsschritt verlangen, was der
   gesamten "einmal dekodieren, beliebig oft günstig entwickeln"-
   Architektur widerspräche. Dieselbe Scope-Grenze, die `gamut-dng`
   selbst für sich zieht ("Farbwiedergabe ist Aufgabe eines
   Rohdaten-Prozessors").
2. **`ProfileHueSatMapDims`/`ProfileHueSatMapData1`/`ProfileToneCurve`**
   (die "Look"-Daten, die z. B. Adobes "Camera Landscape" von "Camera
   Standard" unterscheiden) — wird **echt angewendet**, in
   `stages::calibration`, anstelle der bisherigen Handlisten-Näherung.
   Die PDF-Spezifikation selbst war von dieser Sandbox aus nicht
   abrufbar (`docs.rs`, `huggingface.co`, `helpx.adobe.com`,
   `paulbourke.net` — alle blockiert), aber Adobes eigenes DNG-SDK ist
   auf GitHub quelloffen (`raw.githubusercontent.com` war erreichbar):
   Indexierung, Interpolationsformel (trilinear, Farbton zirkulär) und
   die HSV-Parametrisierung (`0.0..6.0` statt Grad) in
   `stages::calibration::apply_hue_sat_map` sind eine direkte Portierung
   von `dng_reference.cpp::RefBaselineHueSatMap` (samt
   `dng_hue_sat_map.h/.cpp` für die Tabellen-Indexierung und
   `dng_utils.h::DNG_RGBtoHSV/DNG_HSVtoRGB` für die HSV-Umrechnung,
   letztere als `stages::color_math::rgb_to_hsv6`/`hsv6_to_rgb` portiert)
   — nicht aus dem Gedächtnis nachgebaut. Bewusst ausgelassen: das
   optionale `ProfileHueSatMapEncoding` (nichtlineare Wert-Kodierung, nur
   bei wenigen Profilen gesetzt) und die Spline-Interpolation der
   Tonwertkurve (stückweise linear stattdessen, siehe `apply_tone_curve`s
   Kommentar).

**CPU-only:** die HueSatMap-Tabelle ist variabel groß (bis zu Hunderten
Einträgen), passt nicht in `calibration.rs`s festes GPU-Uniform-Layout —
läuft aus demselben Grund CPU-seitig wie `ContentAwareFill`/`AiInpaint`
(`stages::repair`).

**Entscheidung:** Neues additives `CalibrationAdjustment::dcp_profile:
Option<DcpProfileData>` (dasselbe "einmal auflösen, als Zahlen im EDL
ablegen"-Muster wie `AiFillPatch`, Phase 13 Schritt 1) — hat Vorrang vor
`camera_profile`s Handliste, die als Fallback bestehen bleibt, wenn kein
`.dcp` importiert wurde. `apx-app`s `import_dcp_profile`-Command öffnet
einen Datei-Dialog und parst — kein eingebautes Profil im Installer,
derselbe Opt-in wie beim LensFun-Kalibrier-Assistenten.

## ADR-0040-Nachtrag II: Schritt 4 — echte Perspektive/Upright-
Kantenerkennung (`imageproc`); zwei erkannte Effekte statt vier
unabhängiger, `Full` bewusst identisch zu `Auto`

**Status:** Angenommen
**Kontext:** `stages::lens_corrections`s vier Upright-Automatikmodi
(`Auto`/`Level`/`Vertical`/`Full`) waren seit Phase 4 (ADR-0028)
dokumentierte No-op-Platzhalter — wählbar, ohne Wirkung. Nur der
„Guided"-Modus tat etwas: der Nutzer zieht selbst zwei Hilfslinien, ihr
gemittelter Neigungswinkel wird zur Dreh-Korrektur.

**Umsetzung:** `apx-ai::upright` (neues Modul, klassische CV ohne
gelerntes Modell — dieselbe Handschrift wie `lens_calibration`, Phase 12
Schritt 3 Teil B): `imageproc::edges::canny` findet Kanten,
`imageproc::hough::detect_lines` findet darin gerade Linien
(Normalform-Winkel `0..180°`). Statt vier unabhängige Automatiken zu
bauen, macht das Modul **zwei echte Effekte** real und kombiniert sie je
nach Modus:
- **Level** (nahezu waagerechte erkannte Linien mitteln → `rotate_degrees`)
  — exakt dieselbe Rechnung wie `guided_rotation_degrees`, nur mit
  automatisch gefundenen statt vom Nutzer gezogenen Linien.
- **Vertical** (nahezu senkrechte erkannte Linien mitteln →
  `manual_transform.horizontal`-Scherung, die trotz des Namens die
  *senkrechte* Kantenkonvergenz korrigiert) — die Zuordnung
  Winkelabweichung → Scherungsreglerwert ist direkt aus
  `lens_corrections.rs`s bestehender `undo_manual_transform`-Formel
  hergeleitet (Koeffizientenvergleich, nicht geraten), und per
  Vorzeichen-/Größenprobe an einer synthetischen Testkante nachgerechnet
  (`upright.rs`s Testmodul) statt nur symbolisch behauptet — ein früherer
  Testentwurf mit fehlerhafter Verifikationsformel hätte sonst einen
  Vorzeichenfehler nicht aufgedeckt.
- **Auto**/**Full**: beide Effekte kombiniert. Eine echte Trennung
  zwischen Adobes moderater "Auto"-Automatik und einer vollen
  Vier-Parameter-"Full"-Homografie bräuchte eine echte
  Homografie-Schätzung aus mehreren, unabhängig konvergierenden
  Linienscharen — außerhalb des Umfangs des bereits in ADR-0028/-0030 auf
  ein einziges Scherungspaar vereinfachten Objektivkorrektur-Modells.
  `Full` verhält sich deshalb bewusst identisch zu `Auto`, statt eine
  durch die vorhandenen Daten nicht gedeckte zusätzliche Korrektur zu
  erfinden — dieselbe Ehrlichkeit wie Schritt 3s unangewandte
  DCP-Farbmatrix.

**Architektur:** `apx-ai` (nicht `apx-pipeline`) bekommt die neue
`imageproc`-Abhängigkeit (`default-features = false`, nur `rayon` —
`text`/`fft` unnötig), da `apx-ai` bereits von `apx-pipeline` abhängt
(nicht umgekehrt) und `lens_calibration` als Vorbild ebenfalls dort
lebt. `apx-app`s `detect_upright_correction`-Command folgt demselben
"Analyse-Auflösung über `TileCache` dekodieren"-Muster wie
`generate_ai_mask`, obwohl es keine KI-Funktion im engeren Sinn ist.
Frontend übernimmt nur die zum gewählten Modus passende Komponente in
`manual_transform` (Level nur Rotation, Vertical nur Scherung) statt
beide Felder blind zu überschreiben — sonst würde ein Klick im
"Level"-Modus eine zuvor manuell gesetzte Scherungskorrektur
stillschweigend auf 0 zurücksetzen.

## ADR-0040-Nachtrag III: Schritt 5 — Panorama-Homografie-Stitching;
das geplante rust-cv-Ökosystem kompiliert auf keinem aktuellen
stabilen Rust, `imageproc`-FAST-Ecken + eigener BRIEF-Deskriptor +
`homography`-Crate stattdessen

**Status:** Angenommen
**Kontext:** ADR-0040 hatte für Panorama-Stitching das `rust-cv`-
Ökosystem (`akaze`, `cv-core`, `space`, `arrsac`, `sample-consensus`)
als real verfügbar eingestuft — per `cargo add --dry-run` gegen die
echte crates.io-Registry bestätigt. Dieser Schritt hat einen echten
Einbindungsversuch unternommen, und `cargo add --dry-run` erwies sich
als unzureichende Prüfung: es bestätigt nur, dass sich die Abhängigkeits-
Metadaten auflösen lassen, nicht dass der resultierende Code tatsächlich
kompiliert.

**Der reale Befund:** `akaze` (letzte Veröffentlichung 2021) hängt
transitiv an `bitarray 0.2`, dessen `src/lib.rs` unbedingt
`#![feature(min_const_generics)]` setzt. Dieses Attribut selbst — nicht
das längst stabile Feature dahinter — verlangt einen Nightly-Compiler
und schlägt auf jedem aktuellen stabilen Rust mit `error[E0554]` fehl,
real gegen `rustc 1.94.1` in dieser Sandbox getestet (`cargo test`
brach mit exakt diesem Fehler ab). Das ganze `rust-cv`-Ökosystem ist
damit auf stabilem Rust praktisch tot — unabhängig von den zusätzlich
noch bestehenden Versions-Inkompatibilitäten, die bereits vor diesem
Befund auffielen: `cv-core 0.15`/`nalgebra 0.21`/`space 0.10` (alle vom
selben Autor, derselben Ära) vs. der aktiver gepflegten `homography`-
Crate mit `nalgebra 0.33`; `homography` hängt zudem gar nicht von
`sample-consensus` ab (kein `Estimator`-Trait-Impl, per `grep` in
dessen `Cargo.toml` bestätigt) — `arrsac`/`sample-consensus`/`cv-core`/
`space` hätten also ohnehin keinen echten Klebstoff zwischen `akaze`
und `homography` geliefert.

**Umsetzung stattdessen** (`apx-stacking::homography_stitch`, neues
Modul, siehe dessen ausführliche Moduldoku): `imageproc::corners::
corners_fast9` (dieselbe Crate, bereits in Schritt 4 real erprobt) für
die Eckenerkennung; ein selbst geschriebener BRIEF-artiger 256-Bit-
Deskriptor (klassische, publizierte Technik — Calonder et al. 2010,
nicht fabriziert) statt eines externen Deskriptor-Crates; brute-force
Hamming-Abstands-Matching mit Lowes Verhältnistest; ein eigener, kurzer
RANSAC-Loop (Zufallsstichprobe → `homography`-Crates echte DLT-Schätzung
per SVD → Inlier zählen → beste Stichprobe über alle Inlier
verfeinern) statt der generischen `sample_consensus`/`arrsac`-
Maschinerie. `homography` bekommt dieselbe `nalgebra`-Version (`0.33`)
explizit in `apx-stacking` UND `apx-app` gepinnt — dieselbe Instanz im
Abhängigkeitsgraph statt einer separaten Kopie, sonst wäre
`homography`s `Matrix3<f64>` in eigenem Code ein anderer Typ.

**Test-Vorsicht mit synthetischen Bildern:** ein Schachbrettmuster als
Testbild schlug fehl — an dessen Kreuzungen treffen sich vier
Quadranten in einem X-förmigen Sattelpunkt, den FAST grundsätzlich
nicht erkennt (es braucht einen einzigen zusammenhängenden Bogen von
mindestens neun der 16 Kreispunkte). Ein perfekt periodisches
Punktraster schlug ebenfalls fehl — Lowes Verhältnistest verwirft zu
Recht mehrdeutige Treffer, wenn viele Merkmale identische lokale
Nachbarschaften haben. Beide Male kein Bug im Code, sondern ein
unrealistisches Testbild; die endgültigen Tests nutzen verstreute,
unterschiedlich große Quadrate an leicht versetzten Positionen.

**Entscheidung:** `apx-app`s `stack_panorama`-Command versucht das
echte Homografie-Stitching zuerst und fällt für die gesamte Fotoserie
auf die bestehende reine Verschiebungs-Registrierung
(`apx_stacking::panorama`, Phasenkorrelation) zurück, wenn für
mindestens ein Foto keine verlässliche Homografie gefunden wird —
bewusst alles-oder-nichts statt einer Mischkomposition aus beiden
Positionierungsarten auf derselben Leinwand.

## ADR-0040-Nachtrag IV: Schritt 6 — Mehrere Kataloge + Katalog-Wartung;
Katalogwechsel per Neustart statt Hot-Swap der offenen Verbindung

**Status:** Angenommen
**Kontext:** `AppState::catalog: Arc<Catalog>` wird in `commands.rs` von
praktisch jedem einzelnen Tauri-Command direkt referenziert oder
geklont. Ein echtes Hot-Swap der offenen Katalogverbindung im
laufenden Prozess hätte entweder jeden dieser Zugriffe hinter ein
zusätzliches Lock verlegt (invasiv, hohes Fehlerrisiko quer durch eine
~6000-Zeilen-Datei) oder den `Arc` selbst austauschbar gemacht
(dieselbe Umbau-Größe) — für einen einzelnen Schritt nicht vertretbar.

**Entscheidung:** dieselbe UX wie Adobe Lightroom Classics eigener
Katalogwechsel ("Diese Änderung erfordert einen Neustart"): Wechseln
oder Neuanlegen eines Katalogs speichert den Zielpfad in
`Settings::catalog` und ruft `AppHandle::request_restart()` — die App
startet neu und öffnet beim nächsten Start automatisch den neuen
Pfad. Dafür genügt eine einzige, unauffällige Änderung an `main.rs`
(liest `settings.catalog.last_opened_catalog` vor dem `Catalog::open`-
Aufruf) statt eines Umbaus der gesamten Command-Schicht.

**Fund:** `Settings::catalog::last_opened_catalog` existierte bereits
seit Phase 10 (Settings-Fundament) im Datenmodell, wurde aber nie
gelesen — `main.rs` öffnete unbedingt `paths.default_catalog_file()`.
Reine Attrappe, jetzt tatsächlich verdrahtet (mit Rückfall auf den
Standardkatalog, falls der hinterlegte Pfad seit dem letzten Start
verschoben/gelöscht wurde — kein Absturz beim Start wegen eines
veralteten Pfads).

**Katalog-Wartung** braucht dagegen keinen Neustart — sie arbeitet auf
der bereits offenen Verbindung: `apx_catalog::Catalog` bekommt
`integrity_check` (`PRAGMA integrity_check`, SQLites eigene
Standardprüfung auf strukturelle Schäden), `vacuum` (`VACUUM`) und
`backup_to` (SQLites Online-Backup-API über `rusqlite`s `backup`-
Feature — sicher neben der weiterhin offenen Verbindung nutzbar,
anders als eine rohe Dateikopie, die bei gleichzeitigem Schreibzugriff
eine inkonsistente Kopie ergeben könnte).

**Kein neuer Dialog-Wechsel-Mechanismus fürs Frontend nötig:** die
generischen `pick_file_path`/`pick_save_file_path`-Commands (bereits
für Drucken/Buch/ICC-Profil-Auswahl vorhanden) übernehmen die
Datei-Dialoge für "Neuer Katalog…"/"Katalog öffnen…"/"Sichern unter…"
direkt — die neuen Rust-Commands nehmen nur noch den bereits gewählten
Pfad entgegen.

## ADR-0040-Nachtrag V: Schritt 7 — echter UND/ODER-Regelbaum für
bedingte Presets und intelligente Sammlungen; in-memory statt
dynamischer SQL-Generierung

**Status:** Angenommen
**Kontext:** Zwei getrennte Stellen kannten bisher nur eine flache,
ausschließlich UND-verknüpfte Regelliste: bedingte Presets
(`PresetCondition[]`, ADR-0031 Punkt 4, "kein ODER, keine
Verschachtelung") und intelligente Sammlungen (`apx_catalog::
FilterCriteria`, feste Struktur-Felder Bewertung/Flagge/Farbe/
Kameramodell, ADR-0023). Beide werden bereits als opakes JSON
gespeichert (`conditions_json`/`smart_criteria_json`) — kein
Datenbankschema-Wechsel nötig, nur ein neues JSON-Schema.

**Entscheidung:** ein gemeinsamer, generischer Regelbaum-Typ
(`RuleNode<TLeaf> = { type: "condition"; condition: TLeaf } |
{ type: "group"; operator: "and" | "or"; children: RuleNode<TLeaf>[] }`,
`frontend/src/lib/ruleTree.ts`) mit einer rekursiven Auswertung
(`evaluateRuleNode`) und einem generischen Editor
(`RuleTreeEditor.tsx`, kennt das Blatt-Vokabular nicht — kommt per
`renderLeaf`/`makeDefaultLeaf`-Props vom Aufrufer). Dasselbe
JSON-Schema existiert auf der Rust-Seite als `apx_catalog::FilterNode`
(`#[serde(tag = "type", rename_all = "snake_case")]`) — bewusst
identisch zum TypeScript-Gegenstück gewählt, damit `create_smart_
collection` den vom Frontend erzeugten JSON-String unverändert als
opakes `criteria_json` durchreichen und rein per `serde_json`
parsen kann, ganz ohne eigene Übersetzungsschicht zwischen den beiden
Seiten.

**Bedingte Presets bleiben reines Frontend:** `conditions_json` ist
für `apx-catalog` schon vorher ein opaker String gewesen (unverändert:
`Preset.conditions_json`), also brauchte dieser Teil **keine
Rust-Änderung**. `PresetCondition`/`evaluateCondition`/
`parseConditions`/`applyConditionsToSubset` (`lib/presets.ts`) bleiben
unverändert bestehen — sie werden weiterhin von den Auto-
Verschlagwortungsregeln (`MetadataDialog.tsx`s `createTagRule`, ein
eigenes, außerhalb dieses Schritts liegendes Feature) benutzt und
dienen als Migrationsquelle. Neu: `PresetRuleGroup { section:
PresetSectionKey | null; node: RuleNode<PresetLeafCondition> }` — die
Sektions-Gatter-Semantik (`section: null` = ganzes Preset, sonst nur
diese Sektion; mehrere Regelgruppen bleiben untereinander
UND-verknüpft) bleibt exakt wie zuvor, nur ist jede einzelne Regel
jetzt ein ganzer UND/ODER-Baum statt einer einzelnen Bedingung.
`parseRules` akzeptiert die neue Baumform direkt und migriert sonst
über `migrateLegacyConditions` von der alten flachen Form — jede
vorhandene, vor diesem Schritt gespeicherte Preset-Bedingung bleibt
dadurch ohne Migrationsskript lesbar.

**Intelligente Sammlungen werten in-memory aus, nicht per dynamischer
SQL-Generierung:** `apx_catalog::FilterNode::matches(&Photo) -> bool`
wird für jedes Foto einzeln aufgerufen, nachdem `repository::
collections::list_photos` den gesamten Fotobestand bereits per SQL
geladen hat (`filter_photos` mit leeren Kriterien, dieselbe Abfrage
wie die Filterleiste). Eine zweite, rekursive WHERE-Klausel-
Generierung neben der bestehenden in `repository::search::
build_filter_clause` wäre für beliebig tiefe Verschachtelung deutlich
aufwendiger gewesen, ohne einen echten Nutzen — Kataloge in diesem
Projekt sind Einzelnutzer-Bibliotheken (siehe schon ADR-0040-Nachtrag
III zur Panorama-Homografie: "kein Web-Maßstab"), keine Datenbank mit
Millionen Zeilen, bei der ein voller Tabellenscan pro Sammlung
spürbar würde. `FilterCriteria`/`build_filter_clause` bleiben für die
Filterleiste/Stapelverarbeitungs-Konsole unverändert bestehen — dort
reicht flach UND-verknüpft weiterhin aus, ein Umbau auf den Regelbaum
hätte keinen Mehrwert gebracht.

**Migration alter intelligenter Sammlungen:** `parse_filter_node`
versucht zuerst, `smart_criteria_json` als `FilterNode` zu lesen; bei
fehlendem `"type"`-Tag (vor diesem Schritt gespeicherte, flache
`FilterCriteria`-Form) fällt es auf `FilterCriteria` zurück und
migriert über `impl From<FilterCriteria> for FilterNode` (jedes
gesetzte Feld wird eine Bedingung in einer UND-Gruppe) — dieselbe
"lies alt, migriere beim Zugriff, kein Schreib-Migrationsskript"-
Konvention wie bei den bedingten Presets oben.
