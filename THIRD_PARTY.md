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

Weitere für Phase 8 geprüfte, aber noch nicht direkt eingebundene Abhängigkeiten (siehe ADR-0034, `PLAN.md`-Vermerk bei Schritt 1: erst im jeweiligen Schritt ergänzt, nicht vorab alle auf einmal — `printpdf`s transitiver Font-/Layout-Baum hat in dieser Sandbox das Plattenkontingent tatsächlich erschöpft): `lcms2` (MIT, „static"-Feature), `ab_glyph` (Apache-2.0), `printpdf` (MIT), `suppaftp` (Apache-2.0), `russh`/`russh-sftp` (Apache-2.0), `reverse_geocoder` (MIT), `quick-xml` (MIT OR Apache-2.0) — keine GPL-Kandidaten, werden bei ihrer jeweiligen Einbindung hier mit vollem Eintrag nachgetragen.

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

## Testdaten (`testdata/`)

Werden beim Beschaffen einzeln mit Quelle und Lizenz eingetragen, sobald sie in Phase 1 hinzukommen (Vorgabe: nur frei lizenzierte RAWs, z. B. von raw.pixls.us, CC0). Aktuell: **noch keine Testdateien vorhanden.**

| Datei | Quelle | Lizenz | Kamera/Format |
|---|---|---|---|
| _(noch leer)_ | | | |

---

*Einträge für Phase 2 und später kommen hinzu, sobald die jeweilige Phase startet.*
