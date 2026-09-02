# THIRD_PARTY.md — Abhängigkeiten und Lizenzen

Jede Bibliothek, die dem Projekt hinzugefügt wird, wird hier vor bzw. mit dem Commit eingetragen, der sie einführt. Lizenzprüfung erfolgt beim Hinzufügen, nicht nachträglich gesammelt.

**Regel aus `SPEC.md`:** Nichts mit GPL im Kern, außer ausdrücklich darauf hingewiesen. Ausnahmen sind unten als solche markiert und in `DECISIONS.md` mit ADR begründet.

---

## Rust — geplant für Phase 1

| Crate | Lizenz | Zweck | Hinweis |
|---|---|---|---|
| `rawler` | **LGPL-2.1** | RAW-Dekodierung, Metadaten | **Ausnahme, siehe ADR-0002** — einzige praktikable Rust-Bibliothek mit breiter Formatabdeckung; alle Alternativen (`rawloader`, `quickraw`, `libraw`-Bindings) sind ebenfalls LGPL-2.1 bzw. LGPL/CDDL |
| `kamadak-exif` | BSD-2-Clause | EXIF-Metadaten-Fallback | Unkritisch |
| `image` | MIT OR Apache-2.0 | JPEG/PNG/TIFF-Fallback-Dekodierung | Unkritisch |
| `rusqlite` (Feature `bundled`) | MIT (SQLite selbst: Public Domain) | SQLite-Zugriff | Siehe ADR-0001 |
| `thiserror` | MIT OR Apache-2.0 | Fehlertypen | Unkritisch |
| `directories` | MIT OR Apache-2.0 | Plattform-Pfade | Unkritisch |
| `serde` / `serde_json` / `toml` | MIT OR Apache-2.0 | Serialisierung, Settings | Unkritisch |
| `tracing` / `tracing-subscriber` | MIT | Logging | Unkritisch |
| `uuid` (Feature `v7`) | MIT OR Apache-2.0 | IDs, siehe ADR-0005 | Unkritisch |
| `walkdir` | MIT OR Unlicense | Ordner-Scan beim Import | Unkritisch |
| `rayon` | MIT OR Apache-2.0 | Worker-Pool für Thumbnail-Erzeugung | Unkritisch |
| `tauri` (v2) | MIT OR Apache-2.0 | App-Shell | Unkritisch |
| `time` | MIT OR Apache-2.0 | Zeitstempel/Zeitzonen | Unkritisch |

## Rust — Phase 2

| Crate | Lizenz | Zweck | Hinweis |
|---|---|---|---|
| `wgpu` | MIT OR Apache-2.0 | GPU-Zugriff (Vulkan/Metal/DX12/GL) für die Entwickeln-Pipeline | Unkritisch, siehe ADR-0012 |
| `bytemuck` | Zlib OR Apache-2.0 OR MIT | Sicheres Byte-Layout für GPU-Uniform-Puffer | Unkritisch |
| `pollster` | Apache-2.0 OR MIT | Blockierendes Warten auf wgpus async-API ohne eigene Runtime-Abhängigkeit | Unkritisch |

**Nachträglich entfernt (Schritt 9):** `lcms2` war seit Schritt 1
in `Cargo.toml` eingetragen, wurde aber nie tatsächlich im Code verwendet
— die Kamera→sRGB-Farbtransformation kam am Ende ohne echtes
ICC-Farbmanagement aus (feste Matrix + `srgb_gamma`, siehe ADR-0019),
das ursprünglich für `lcms2` vorgesehene ProPhoto-Arbeitsraum-/
Ausgabeprofil-Feature blieb zurückgestellt (siehe `color/mod.rs`s
Moduldoku). Eine unbenutzte Abhängigkeit baut unnötig eine native
C-Bibliothek mit ein — beim Dokumentations-Check in Schritt 9 aufgefallen
und entfernt, statt sie bis zu einem tatsächlichen Verwendungszeitpunkt
mitzuschleppen. Kommt zurück, sobald ein konkreter Aufrufer für echtes
ICC-Farbmanagement existiert.

## Rust — Phase 3 (Nachtrag Schritt 8)

| Crate | Lizenz | Zweck | Hinweis |
|---|---|---|---|
| `sha2` | MIT OR Apache-2.0 | Streaming-SHA-256-Hash für exakte Duplikaterkennung beim Import | Unkritisch — war schon transitiv im `Cargo.lock` vorhanden (Version 0.10.9), jetzt direkte Abhängigkeit, siehe `DECISIONS.md` ADR-0027 |

## Rust — Phase 8 (Schritt 1, siehe `DECISIONS.md` ADR-0034)

| Crate | Lizenz | Zweck | Hinweis |
|---|---|---|---|
| `ravif` | BSD-3-Clause | AVIF-Encoding (`apx-export`) | Unkritisch — Transitive Abhängigkeit über `image`s `avif`-Feature, keine eigene direkte Abhängigkeit |
| `rav1e` | BSD-2-Clause | AV1-Encoder hinter `ravif` | Unkritisch |
| `image-webp` | MIT OR Apache-2.0 | WebP-Encoding/-Decoding (verlustfrei) hinter `image`s `webp`-Feature | Unkritisch |

## Rust — Phase 8 (Schritt 2, siehe `DECISIONS.md` ADR-0034)

| Crate | Lizenz | Zweck | Hinweis |
|---|---|---|---|
| `lcms2` | MIT | Echtes ICC-Farbmanagement für den Export (`apx_export::icc`) | „static"-Feature bündelt Little-CMS2 (selbst MIT) als C-Quelle, keine Systembibliothek. Zweiter Anlauf nach Phase 1 (siehe Eintrag oben unter „Nachträglich entfernt") — diesmal mit echtem Aufrufer |
| `ab_glyph` | Apache-2.0 | Text-Wasserzeichen-Rasterisierung (`apx_export::watermark`) | Reines Rust, keine Systemschrift-API |
| `ttf-parser` / `owned_ttf_parser` / `ab_glyph_rasterizer` | MIT OR Apache-2.0 / Apache-2.0 / Apache-2.0 | Transitiv über `ab_glyph` | Unkritisch |

Weitere für Phase 8 geprüfte, aber noch nicht direkt eingebundene Abhängigkeiten (siehe ADR-0034, `PLAN.md`-Vermerk bei Schritt 1: erst im jeweiligen Schritt ergänzt, nicht vorab alle auf einmal — `printpdf`s transitiver Font-/Layout-Baum hat in dieser Sandbox das Plattenkontingent tatsächlich erschöpft): `printpdf` (MIT), `suppaftp` (Apache-2.0), `russh`/`russh-sftp` (Apache-2.0), `reverse_geocoder` (MIT), `quick-xml` (MIT OR Apache-2.0) — keine GPL-Kandidaten, werden bei ihrer jeweiligen Einbindung hier mit vollem Eintrag nachgetragen.

## Rust — Phase 9 (siehe `DECISIONS.md` ADR-0035)

**Nachtrag:** Die ersten vier Zeilen unten (`image_hasher`, `rustfft`,
`rhai`, `libloading`) wurden bereits in Schritt 1/8/9 eingeführt, aber
entgegen der oben stehenden Regel nicht sofort hier eingetragen — beim
Verifizieren von Schritt 11 aufgefallen und rückwirkend nachgetragen,
statt stillschweigend übergangen.

| Crate | Lizenz | Zweck | Hinweis |
|---|---|---|---|
| `image_hasher` | MIT OR Apache-2.0 | Perceptual-Hash-Duplikaterkennung (pHash/dHash, Schritt 1) | Unkritisch |
| `rustfft` | MIT OR Apache-2.0 | 2D-Phasenkorrelation für Panorama-/Astro-Stacking (`apx-stacking`, Schritt 8) | Unkritisch, reines Rust |
| `rhai` | MIT OR Apache-2.0 | Skript-API-Engine (`apx-script`, Schritt 9) | Unkritisch, reines Rust, sandboxbar — keine C-Bibliothek wie übliche Lua-Bindings |
| `libloading` | ISC | Plugin-`cdylib`-Laden (`apx-plugin-host`, Schritt 9) | Unkritisch, ISC ist eine permissive MIT-ähnliche Lizenz |
| `gphoto2` | MIT | Tethered Shooting (`apx-tether`, Schritt 11), hinter Cargo-Feature `tethering` (standardmäßig aus) | Bindet an System-`libgphoto2`, das selbst **LGPL-2.1** ist — **Ausnahme, siehe `DECISIONS.md` ADR-0035 Punkt 5**, derselbe Präzedenzfall wie `rawler` oben: keine praktikable Alternative für PTP/USB-Kamerasteuerung aus Rust. Die `gphoto2`-Crate selbst ist MIT, nur die dynamisch gelinkte Systembibliothek ist LGPL-2.1 (dynamisches Linken einer LGPL-Bibliothek verlangt keine Lizenzänderung des aufrufenden Codes) |

## Rust — Phase 11 (siehe `DECISIONS.md` ADR-0038)

| Crate | Lizenz | Zweck | Hinweis |
|---|---|---|---|
| `gamut-dng` | MIT OR Apache-2.0 | Schreiben von „Linear DNG"-Dateien aus kamera-nativen, unentwickelten RAW-Daten (`apx_export::dng`, Schritt 1) | Reines Rust, keine Systembibliothek. Zum Zeitpunkt von ADR-0034 (Phase 8) gab es keine schreibfähige DNG-Crate, diese wurde erst danach verfügbar — per Testbau (Encode→Decode-Rundreise) real verifiziert, nicht nur an der Registry-Beschreibung geglaubt |

## Frontend — geplant für Phase 1

| Paket | Lizenz | Zweck | Hinweis |
|---|---|---|---|
| `react` / `react-dom` (19) | MIT | UI-Framework | Unkritisch |
| `vite` | MIT | Build-Tool | Unkritisch |
| `typescript` | Apache-2.0 | Typsystem | Unkritisch |
| `zustand` | MIT | State-Management | Unkritisch |
| `immer` | MIT | Undo/Redo-Middleware-Basis | Unkritisch |
| `tailwindcss` (4) | MIT | Styling | Unkritisch |
| `@tanstack/react-virtual` | MIT | Virtualisierter Filmstreifen | Unkritisch |
| `@tauri-apps/api` | MIT OR Apache-2.0 | Tauri-Frontend-Bindings | Unkritisch |
| `@tauri-apps/cli` | Apache-2.0 OR MIT | Installer-Bau (`tauri build`), Phase 10 Schritt 11 | Reines Build-Werkzeug (DevDependency), nicht im ausgelieferten App-Bundle enthalten — trotzdem eingetragen (siehe Regel oben: jede hinzugefügte Bibliothek) |

## Testdaten (`testdata/`)

Werden beim Beschaffen einzeln mit Quelle und Lizenz eingetragen, sobald sie in Phase 1 hinzukommen (Vorgabe: nur frei lizenzierte RAWs, z. B. von raw.pixls.us, CC0). Aktuell: **noch keine Testdateien vorhanden.**

| Datei | Quelle | Lizenz | Kamera/Format |
|---|---|---|---|
| _(noch leer)_ | | | |

---

*Einträge für Phase 2 und später kommen hinzu, sobald die jeweilige Phase startet.*
