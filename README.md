## APERTURE X ##

Aperture X is an open source photo and video editor in the tradition of
Adobe Lightroom, Photoshop and DaVinci Resolve. Unlike them, Aperture X
does not rely on a pay-per-use or annual subscription model. It is
licensed under the **Apache License, Version 2.0** (see `LICENSE`) and
is built to be user-focused. The feature list is in `FEATURES.md`.

Development Team

## Lizenz

Aperture X' eigener Code steht unter der **Apache-2.0-Lizenz**
(`LICENSE`). Wer ihn weitergibt, muss `LICENSE` und `NOTICE` mitgeben —
beides liegt dem gebauten Installer bei.

**Zwei Einschränkungen, die „open source" hier nicht vollständig
machen** — sie stehen ausführlich in `NOTICE` und in `DECISIONS.md`
ADR-0063:

1. **GSAP** (Oberflächenanimationen) steht nicht unter einer
   Open-Source-Lizenz, sondern unter GreenSocks eigener Standard-
   „no charge"-Lizenz. Die Nutzung und Weitergabe ist kostenlos
   erlaubt, aber GSAP lässt sich nicht unter Apache-2.0
   unterlizenzieren. Wer Aperture X forkt, ist für diese eine
   Komponente an GreenSocks Bedingungen gebunden.
2. **Drei LGPL-Bibliotheken** — `rawler` (RAW-Dekodierung, LGPL-2.1),
   `lensfun` (Objektivkorrekturen, LGPL-3.0-or-later) und optional
   `gphoto2` (Tethering, LGPL-2.1-only). Für die quelloffene Weitergabe
   ist das unproblematisch: die Austauschbarkeit, die LGPL verlangt, ist
   durch den mitgelieferten Quelltext gegeben. Für eine geschlossene
   Weitergabe wäre sie es nicht.

Die vollständige Lizenzliste aller 952 Rust- und 170 npm-Abhängigkeiten
liegt maschinenlesbar unter `licenses/` und wird von
`tools/license-audit.py` aus den echten Manifesten erzeugt. Ein Test
(`crates/apx-app/tests/bundle_config.rs`) schlägt an, sobald eine
Abhängigkeit dazukommt, die dort nicht erfasst ist.

## Installer bauen und veröffentlichen

Siehe `RELEASE.md` — welche Pakete auf welcher Plattform entstehen,
welche Secrets für die Code-Signierung nötig sind, und was passiert
(bzw. nicht passiert), solange keine hinterlegt sind.
