# Aperture X — verbindliche Arbeitsregeln

Tauri 2 + React/TypeScript + Rust. 13 Crates unter `crates/`, Frontend
unter `frontend/`. **Alle Befehle laufen im Wurzelverzeichnis**, nicht in
`crates/apx-app`.

Diese Datei ist verbindlich. Ausführliches steht in `docs/` und wird nur
bei Bedarf gelesen:

- `docs/ARBEITSWEISE.md` — Verifikation, Gegenkontrolle, Sitzungsführung
- `docs/BEFEHLE.md` — alle Prüf- und Baubefehle mit Ausgabelängen
- `docs/UMGEBUNG.md` — Plattenquote, pnpm, Tauri-CLI, Playwright, Windows

Einmalig je Rechner: `node tools/setup-claude-global.mjs` legt die
projektübergreifenden Regeln unter `~/.claude/CLAUDE.md` ab. Alles
andere — Kontext-Limit, Leseregeln, der automatische Token-Audit —
steht in `.claude/settings.json` und wirkt ohne Zutun.

## Sprache

Commit-Nachrichten, PR-Texte, ADRs, Code-Kommentare, `PLAN.md`,
`FEATURES.md` und alle Oberflächentexte: **Deutsch**. Rust-Testnamen
bleiben englisch (Bestandskonvention). Kein Modellname in Commits,
PR-Texten, Code-Kommentaren oder sonstigen Repo-Artefakten.

## Token-sparsame Arbeitsweise

- Erst gezielt suchen (Glob/Grep), dann nur die relevanten Zeilen lesen
  (`offset`/`limit`). Keine ganzen Dateien lesen, wenn eine Funktion
  reicht. Nichts erneut lesen, was gerade geschrieben wurde.
- Befehle mit knapper Ausgabe ausführen (Reporter `dot`/silent,
  `--stat`, `--oneline`, `-q`). Lange Logs in eine Datei umleiten und nur
  Fehlerzeilen zeigen — **nicht** nach `tail` pipen, wenn die Ausgabe
  groß ist.
- Gezielt prüfen: erst Einzeltest/Typecheck der geänderten Dateien,
  Vollbuild nur am Ende.
- Breite Recherche an einen `Explore`-Subagenten delegieren, nur das
  Ergebnis zurückgeben lassen.
- Antworten kurz: kein Wiederholen des Diffs, keine Zusammenfassung von
  Offensichtlichem, keine Begründung, die schon als ADR dasteht —
  verlinken.
- Nicht lesen: `node_modules`, `dist`, `target`, `coverage`, Lockfiles,
  generierter Code, `assets/luts/*.cube`,
  `.claude/skills/ui-ux-pro-max/data/`.
- **Grep-Werkzeug statt `grep -r`** — rohes `grep -r` ignoriert
  `.gitignore` und trifft gebaute Artefakte.
- Unabhängige Befehle in **einem** Werkzeugaufruf bündeln. Jeder Aufruf
  kostet eine volle Kontext-Neulesung.
- Skripte in eine Datei schreiben, nicht als Heredoc ins Kommando.
- Bei Aufgabenwechsel: Stand in `PLAN.md` festhalten, dann `/clear`.
  Gemessen: eine Sitzung mit 1.649 Turns kostete 675 Mio. Tokens, weil
  jeder Turn den ganzen Verlauf neu liest.

## Verifikation — nicht verhandelbar

- **Der Exit-Code beweist nichts.** Warnungen zählen, nicht annehmen.
- **Die Gegenkontrolle gehört vor die Schlussfolgerung, nicht hinter den
  Einspruch.** Ein Test muss ohne die Korrektur fehlschlagen. Ein Test,
  der mit und ohne Fix grün ist, behauptet nichts. In einer Sitzung
  dreimal verletzt (ADR-0078, ADR-0080, ADR-0081) — jedes Mal fand es
  der Nutzer, nicht ich.
- **Die Messgröße muss die gestellte Frage beantworten.** „Bildpunkte
  ändern sich" ist keine Antwort auf „sieht der Nutzer es" (ADR-0081).
- Eine plausible Begründung ist keine Messung. „Es gibt nur eine
  Grafikkarte, also schadet Parallelität" klang vernünftig und hat die
  Wartezeit verdoppelt (ADR-0080).
- Nie behaupten, etwas sei geprüft, wenn es nicht geprüft ist. Was
  übersprungen wurde, wird genannt.

## Befehle (Kurzform; Vollständiges in `docs/BEFEHLE.md`)

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings -D clippy::unwrap_used
cargo test -p <crate>                      # bevorzugt
cargo test --workspace > "$LOG" 2>&1       # 118.194 Zeichen — immer umleiten
pnpm --dir frontend run typecheck
pnpm --dir frontend test
pnpm --dir frontend exec playwright test e2e/<datei>.spec.ts
npx --prefix frontend tauri build --no-bundle
```

`cargo tauri` existiert nicht — die Tauri-Kommandozeile ist eine
npm-Abhängigkeit des Frontends.

## Große Dateien — nie vollständig lesen

| Datei | Zeichen | ~Tokens |
| --- | --- | --- |
| `DECISIONS.md` (+ `docs/DECISIONS-ARCHIV.md`) | 528.000 gesamt | ~151.000 |
| `crates/apx-app/src/commands.rs` | 461.618 | ~132.000 |
| `frontend/src/store/index.ts` | 338.892 | ~97.000 |
| `PLAN.md` | 303.172 | ~87.000 |
| `frontend/e2e/tauri-mock.ts` | 126.595 | ~36.000 |
| `frontend/src/components/DevelopPanel.tsx` | 115.004 | ~33.000 |

Immer mit `offset`/`limit` oder über das Grep-Werkzeug.

## Dokumentationspflicht

Jede Entscheidung mit Begründung als ADR in `DECISIONS.md`, jede Phase
in `PLAN.md`, jede Nutzerfunktion in `FEATURES.md`. **Eine neue Sitzung
muss aus diesen Dateien arbeiten können, nicht aus dem
Gesprächsverlauf** — das ist die Voraussetzung dafür, dass `/clear`
nichts kostet.

## Git

Entwicklung auf dem zugewiesenen `claude/...`-Branch. Pull Request nur
auf ausdrückliche Bitte. `git push -u origin <branch>`, bei
Netzwerkfehlern bis zu viermal mit Wartezeit 2/4/8/16 s.
