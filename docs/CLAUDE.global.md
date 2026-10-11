# Globale Arbeitsregeln (Vorlage für `~/.claude/CLAUDE.md`)

Diese Datei gilt **projektübergreifend**. Sie liegt hier im Repository,
damit sie versioniert ist und nicht verloren geht; wirksam wird sie erst
als `~/.claude/CLAUDE.md`. Ein Befehl, im Wurzelverzeichnis:

```sh
node tools/setup-claude-global.mjs
```

Er überschreibt nichts: gibt es dort schon eine Datei, legt er die neue
Fassung als `CLAUDE.md.neu` daneben und meldet es.

Alles, was nur für Aperture X gilt, steht in der projekteigenen
`CLAUDE.md` und **nicht** hier — eine globale Datei wird in jedem Projekt
bei jedem Turn mitgelesen und bleibt deshalb kurz. Und alles, was ohne
Kopieren wirken kann, steht in `.claude/settings.json` und kommt mit dem
Klon mit (ADR-0084).

---

## Token-sparsame Arbeitsweise

- Erst gezielt suchen (Glob/Grep), dann nur die relevanten Zeilen lesen
  (`offset`/`limit`). Keine ganzen Dateien lesen, wenn eine Funktion
  reicht. Nichts erneut lesen, was gerade geschrieben wurde.
- Befehle mit knapper Ausgabe ausführen (Reporter `dot`/silent,
  `--stat`, `--oneline`, `-q`, `CI=1`). Lange Logs in eine Datei
  umleiten und nur Fehlerzeilen zeigen.
- Gezielt prüfen: erst Einzeltest/Typecheck der geänderten Dateien,
  Vollbuild nur am Ende.
- Breite Recherche an einen `Explore`-Subagenten delegieren, nur das
  Ergebnis zurückgeben lassen.
- Antworten kurz: kein Wiederholen des Diffs, keine Zusammenfassung von
  Offensichtlichem.
- Nicht lesen: `node_modules`, `dist`, `.next`, `build`, `coverage`,
  Lockfiles, generierter Code.
- Grep-**Werkzeug** statt `grep -r` — rohes `grep -r` ignoriert
  `.gitignore`.
- Unabhängige Befehle in **einem** Werkzeugaufruf bündeln: jeder Aufruf
  kostet eine volle Kontext-Neulesung des gesamten Verlaufs.
- Skripte in eine Datei schreiben, nicht als Heredoc ins Kommando.
- Bei Aufgabenwechsel Sitzung beenden (`/clear`) und den Stand vorher in
  `docs/` oder einer Plan-Datei festhalten.

## Verifikation

- Der Exit-Code beweist nichts — Warnungen zählen, nicht annehmen.
- **Die Gegenkontrolle gehört vor die Schlussfolgerung, nicht hinter den
  Einspruch.** Ein Test, der mit und ohne die Korrektur grün ist,
  behauptet nichts.
- Die Messgröße muss die gestellte Frage beantworten, nicht eine
  verwandte.
- Eine plausible Begründung ist keine Messung.
- Was nicht geprüft wurde, wird genannt — nicht weggelassen.

## Sitzungsökonomie

Gemessen in einem realen Projekt: 1.649 Turns in einer Sitzung =
674.751.012 Tokens Kontext-Neulesung, Ø 409.188 je Turn. Die Fixkosten
(Systemprompt, Werkzeuge, MCP, Skills) waren davon nur 58.600 — der Rest
war Verlauf. Zehn kurze Sitzungen kosten rund ein Drittel von einer
langen.

- Bei über 20 % Kontextauslastung (`/context`) schneiden.
- Nach jedem abgeschlossenen Vorhaben `/clear`.
- Aufgabenlisten abräumen: eine veraltete Liste wird in jedem weiteren
  Turn mitgelesen.
- Mechanische Arbeit (Formatieren, Routine-Git, Dateilisten,
  Commit-Entwürfe) gehört in einen Subagenten mit kleinem Modell.

## Ausgabe

Kein Modellname in Commits, Pull-Request-Texten, Code-Kommentaren oder
sonstigen Artefakten, die in ein Repository wandern.
