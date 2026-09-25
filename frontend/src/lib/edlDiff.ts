import { ALL_SLIDER_SPECS_BY_KEY } from "./edl";
import type { EdlPayload } from "./edl";

/**
 * Was ein Bearbeitungsschritt geändert hat (Phase 34 F6, siehe
 * `DECISIONS.md` ADR-0070).
 *
 * **Wozu.** Die Verlaufs-Zeitachse listet Schritte mit Beschriftungen
 * wie „Belichtung geändert" — aber nicht, *worauf*. Wer nach zwanzig
 * Schritten wissen will, wo eine Einstellung hergekommen ist, muss
 * bisher hin- und herspringen und die Regler ablesen. Dieser Vergleich
 * beantwortet es direkt.
 *
 * **Warum ein allgemeiner Vergleich und keine Liste bekannter Felder.**
 * Das EDL hat über hundert Felder in zwei Dutzend Gruppen und wächst mit
 * jeder Phase. Eine handgepflegte Liste wäre schon beim nächsten neuen
 * Regler unvollständig — und zwar unsichtbar, weil ein fehlendes Feld
 * einfach nicht erscheint. Der Vergleich läuft deshalb über die
 * tatsächliche Struktur.
 *
 * **Warum Listen nur gezählt werden.** Masken, Reparaturstriche und
 * Kurvenstützstellen sind Listen von Objekten; ein Unterschied darin
 * Punkt für Punkt aufzuzählen ergäbe hunderte Zeilen, die niemand
 * liest. „3 → 5 Einträge" sagt, was passiert ist.
 */

export interface EdlChange {
  /** Pfad im EDL, z. B. `basic.exposure_ev`. */
  path: string;
  /** Lesbare Beschriftung, wenn der Pfad zu einem bekannten Regler
   * gehört — sonst der Pfad selbst. */
  label: string;
  before: string;
  after: string;
}

/** Wie viele Nachkommastellen eine Zahl in der Anzeige bekommt. */
function formatValue(value: unknown): string {
  if (value === null || value === undefined) return "—";
  if (typeof value === "boolean") return value ? "an" : "aus";
  if (typeof value === "number") {
    return Number.isInteger(value) ? String(value) : value.toFixed(2);
  }
  if (Array.isArray(value)) {
    return `${value.length} ${value.length === 1 ? "Eintrag" : "Einträge"}`;
  }
  if (typeof value === "object") return "geändert";
  return String(value);
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Beschriftung für einen EDL-Pfad.
 *
 * Gesucht wird über den LETZTEN Pfadabschnitt in den Regler-Angaben
 * (`BASIC_SLIDER_SPECS` und die übrigen) — dieselbe Quelle, aus der das
 * Entwickeln-Panel seine Beschriftungen nimmt. Ein eigener Satz
 * Beschriftungen daneben würde beim nächsten Umbenennen auseinanderlaufen.
 */
export function labelForPath(path: string): string {
  const leaf = path.split(".").pop() ?? path;
  return ALL_SLIDER_SPECS_BY_KEY.get(leaf)?.label ?? path;
}

/**
 * Alle Unterschiede zwischen zwei Bearbeitungsständen, nach Pfad
 * sortiert.
 *
 * Gleitkommawerte werden mit einer kleinen Toleranz verglichen: ein
 * Unterschied unterhalb der Reglerauflösung ist keine Änderung, die
 * jemand vorgenommen hat, sondern Rundung aus dem JSON-Durchlauf.
 */
/**
 * Strukturgleichheit mit fruehem Abbruch — ohne `JSON.stringify`.
 *
 * Der erste Entwurf verglich Listen ueber `JSON.stringify(a) ===
 * JSON.stringify(b)`. Das ist fuer kleine Werte bequem und fuer das
 * echte EDL untragbar: eine 3D-LUT-Tabelle oder eine Maske mit
 * eingebetteten Bilddaten erzeugt dabei zweimal eine Zeichenkette von
 * mehreren Megabyte. Im Browserlauf ist der Renderer daran
 * abgestuerzt ("Page crashed"), noch bevor eine einzige Zeile zu sehen
 * war. Hier wird stattdessen elementweise verglichen und beim ersten
 * Unterschied abgebrochen.
 */
function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a === "number" && typeof b === "number") {
    return Math.abs(a - b) <= EPSILON;
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i += 1) {
      if (!deepEqual(a[i], b[i])) return false;
    }
    return true;
  }
  if (isPlainObject(a) && isPlainObject(b)) {
    const keysA = Object.keys(a);
    const keysB = Object.keys(b);
    if (keysA.length !== keysB.length) return false;
    for (const key of keysA) {
      if (!Object.hasOwn(b, key)) return false;
      if (!deepEqual(a[key], b[key])) return false;
    }
    return true;
  }
  return false;
}

/** Unterschiede unterhalb dieser Schwelle sind Rundung, keine
 * Bearbeitung, die jemand vorgenommen hat. */
const EPSILON = 1e-6;

/**
 * Alle Unterschiede zwischen zwei Bearbeitungsständen, nach Pfad
 * sortiert.
 */
export function diffEdlPayloads(before: EdlPayload, after: EdlPayload): EdlChange[] {
  const changes: EdlChange[] = [];

  const record = (path: string, a: unknown, b: unknown): void => {
    changes.push({
      path,
      label: labelForPath(path),
      before: formatValue(a),
      after: formatValue(b),
    });
  };

  const walk = (a: unknown, b: unknown, path: string): void => {
    if (isPlainObject(a) && isPlainObject(b)) {
      const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
      for (const key of [...keys].sort()) {
        walk(a[key], b[key], path ? `${path}.${key}` : key);
      }
      return;
    }
    // Listen werden als Ganzes gemeldet, nicht Eintrag fuer Eintrag
    // (siehe Moduldoku).
    if (deepEqual(a, b)) return;
    record(path, a, b);
  };

  walk(before as unknown, after as unknown, "");
  return changes;
}
