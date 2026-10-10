# Arbeitsweise — Verifikation, Gegenkontrolle, Sitzungsführung

Diese Datei erklärt das *Warum* hinter den Regeln in `CLAUDE.md`. Sie
wird nur gelesen, wenn eine Regel begründet werden muss.

## Warum es diese Datei gibt

Ein Token-Audit dieses Projekts hat gemessen: **eine** Sitzung mit 1.649
Turns kostete **674.751.012** Tokens Kontext-Neulesung, im Schnitt
**409.188** pro Turn, Spitze 783.223. Die Fixkosten (Systemprompt,
Werkzeuge, MCP, Skills) waren dabei nur 58.600 Tokens — 14 %. Der Rest
war Gesprächsverlauf.

Daraus folgen zwei Dinge:

1. **Jeder Werkzeugaufruf kostet eine volle Kontext-Neulesung.** 918
   Bash-Aufrufe in einer Sitzung bei 409k Kontext sind 375 Mio. Tokens
   nur für Werkzeugrunden. Bündeln ist deshalb kein Stil, sondern der
   zweitgrößte Hebel.
2. **Die Sitzungslänge ist der größte Hebel.** Zehn Sitzungen à 165
   Turns kosten rund ein Drittel von einer Sitzung à 1.649 Turns, weil
   der Kontext nicht mitwächst.

## Die Gegenkontrolle

**Regel: Die Gegenkontrolle gehört vor die Schlussfolgerung, nicht
hinter den Einspruch des Nutzers.**

Drei belegte Verstöße aus einer Sitzung:

**ADR-0078 — zwei untaugliche Beweise.** Behauptung: „die Luftballons
sind sichtbar". Beweis 1 war `document.elementFromPoint` auf der
Ballonmitte — die Schicht trägt `pointer-events: none`, der Strahl geht
also immer hindurch, unabhängig davon, was man sieht. Beweis 2 war „zwei
Aufnahmen unterscheiden sich" — dieser Test war **mit dem Fehler
ebenfalls grün**. Erst eine isolierte Pixelzählung (2.462 gegen 8.909
von 1.440.000) sagte etwas.

**ADR-0080 — Begründung statt Messung.** Eine Schranke sollte überholte
Renderläufe aussortieren und hielt dabei ein Schloss. Begründung: „es
gibt nur eine Grafikkarte, also schadet Parallelität." Plausibel, und
falsch: eine Reihenfolge bricht den laufenden Lauf nicht ab, also
wartete die neueste Anfrage erst auf einen veralteten und rechnete dann
selbst. Gemessen beim Nutzer: 4216 → **8879 ms**. Eine Messung der
Wartezeit der neuesten Anfrage hätte das vorher gezeigt.

**ADR-0081 — richtige Messung, falsche Messgröße.** Der Pixelzähl-Test
aus ADR-0078 war korrekt und gegenkontrolliert. Er misst nur die falsche
Sache: 8.909 von 1.440.000 Bildpunkten sind 0,62 % des Fensters bei 22 %
Deckkraft. Messbar, nicht sichtbar. Die Frage war „sieht der Nutzer
es?", beantwortet wurde „ändert sich etwas im Bild?".

**Checkliste vor jeder Behauptung:**

1. Schlägt der Test ohne die Korrektur fehl? Wenn nein, behauptet er
   nichts.
2. Beantwortet die Messgröße die gestellte Frage, oder nur eine
   verwandte?
3. Ist das eine Messung oder eine Begründung?
4. Was genau wurde nicht geprüft? Das wird genannt, nicht weggelassen.

## Sitzungsführung

- **Nach jedem gemergten PR: `/clear`.** Der Verlauf dazu ist danach
  wertlos und wird sonst hunderte Turns weiter mitgelesen.
- **Bei über 20 % Kontextauslastung:** Stand in `PLAN.md` festhalten,
  dann schneiden. `/context` zeigt die Auslastung.
- **Wissen in Dateien, nicht im Verlauf.** `DECISIONS.md` (97 ADRs) und
  `PLAN.md` (41 Phasen) sind so geschrieben, dass eine frische Sitzung
  daraus arbeiten kann. Das ist der Grund, warum `/clear` hier nichts
  kostet — aber nur, solange die Dateien gepflegt bleiben.
- **Injizierte Erinnerungen sind nicht einmalig.** Eine Skills- oder
  Aufgabenliste, die in den Verlauf injiziert wird, wird danach in jedem
  weiteren Turn mitgelesen. Gemessen: 15 Injektionen der Skills-Liste
  (je 7.400 Tokens) und 105 der Aufgabenliste. Eine veraltete
  Aufgabenliste kostet also dauerhaft — abgeschlossene Vorhaben werden
  abgeräumt.

## Modellwahl

Mechanische Arbeit (Formatieren, Routine-Git, Dateilisten,
Commit-Entwürfe, breites Suchen) braucht nicht das teuerste Modell. Ein
`Explore`- oder `general-purpose`-Subagent mit `model: "haiku"` oder
`"sonnet"` erledigt sie, und seine Zwischenergebnisse landen nicht im
Hauptkontext. Architektur, Fehlersuche und alles, wo eine falsche
Schlussfolgerung teuer ist, bleibt beim großen Modell.

Gemessen: 0 Subagenten in 1.649 Turns, bei 88 `grep -r`-Durchläufen und
einem Audit über „welche UI-Elemente funktionieren nicht". Genau diese
Arbeit gehört in einen Subagenten.

## Messroutine

Am Sitzungsende `/context` und `/cost` notieren. Für die Auswertung der
Transkripte liegt ein Skript bereit, das nur Statistik ausgibt und keine
Inhalte: Tokens je Turn, die größten Werkzeugergebnisse, mehrfach
gelesene Dateien, Kompaktierungen. Abnahmekriterium für jede
Spar-Maßnahme: `Turns × Ø-Kontext` sinkt **und** die Zahl der
Korrekturschleifen steigt nicht. Sinkt der Verbrauch, aber die Nacharbeit
steigt, war es keine Einsparung.
