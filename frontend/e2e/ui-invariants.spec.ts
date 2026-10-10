/**
 * **UI-Invarianten** — die zwei Zusicherungen, die in dieser App am
 * häufigsten UNBEMERKT gebrochen wurden (Phase 35, siehe `DECISIONS.md`
 * ADR-0072).
 *
 * **Wozu.** Beide Fehlerklassen hier haben eine Eigenschaft, die sie
 * besonders teuer macht: sie erzeugen keinen Fehler. Kein roter Rand,
 * keine Konsolenmeldung, kein fehlgeschlagener Test — die Oberfläche
 * sieht vollständig aus und tut nur nichts. Genau deshalb standen sie
 * teils über ein Dutzend Phasen im Programm:
 *
 * 1. **Die Kaskadenfalle.** Ungeschichtetes CSS gewinnt in Tailwind v4
 *    gegen `@layer utilities`, unabhängig von der Spezifität.
 *    `.apx-glass`/`.apx-glass-strong` setzten `position: relative`
 *    ungeschichtet — jede `fixed`/`absolute`/`sticky`-Utility auf einer
 *    Glasfläche verlor also still. Fünf eigenständige Fehler gingen
 *    darauf zurück (ADR-0055, -0055 Nachtrag III, -0056, -0071 und das
 *    Überlaufmenü selbst, dessen erste sechs Einträge dadurch ausserhalb
 *    des Fensters lagen). Dieser Test misst die ERRECHNETE Position und
 *    hält damit die Ursache fest, nicht die fünf Symptome.
 *
 * 2. **Einträge, die nichts tun.** Ein Menüeintrag, der klickbar ist und
 *    beim Klick nichts auslöst, ist aus Benutzersicht ein Totalausfall,
 *    aus Testsicht aber unauffällig: `click()` wirft nicht. Dieser Test
 *    geht JEDEN Eintrag des Überlaufmenüs durch und verlangt genau eines
 *    von beidem — eine messbare Wirkung oder `disabled`.
 *
 * **Warum der IPC-Mitschnitt.** Zwei Einträge wirken korrekt, ohne das
 * DOM zu ändern: „Import mit Vorlage…" öffnet einen Betriebssystem-
 * Ordnerdialog und „Zweites Display…" ein zweites Tauri-Fenster. Beides
 * kann der Mock nicht zeigen, beides ist aber am IPC-Aufruf messbar.
 * Ohne dieses Signal müsste der Test sie ausnehmen — und eine Ausnahme
 * ist genau die Lücke, durch die der nächste echte Ausfall schlüpft.
 * Der Mitschnitt vergleicht gegen eine Grundlinie aus einem gleich
 * langen Ruhefenster vor dem Klick, damit Hintergrund-Abfragen (etwa
 * `catalog_status`) nicht als Wirkung zählen.
 */
import { test, expect } from "@playwright/test";
import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000101",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  missing: false,
  camera_model: "Canon EOS",
  content_hash: "hash-invarianten-1",
  captured_at: "2024-05-07T12:00:00Z",
};

/** Ruhefenster, in dem eine Wirkung sichtbar geworden sein muss. */
const SETTLE_MS = 450;

/** Positionierungs-Utilities, die eine ungeschichtete Regel schlucken kann. */
const POSITION_UTILITIES = ["fixed", "absolute", "sticky"] as const;

async function bootWithPhoto(page: import("@playwright/test").Page): Promise<void> {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Urlaub", photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
    // Ein bestätigter Ordnerdialog, damit „Import mit Vorlage…" nicht
    // bloss den Abbruchfall durchläuft.
    selectFolderResult: "/home/user/Fotos/Neuer Import",
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
}

/**
 * Jedes Element mit einer Glas-Klasse UND einer Positionierungs-Utility,
 * dessen errechnete Position nicht zur Utility passt.
 */
async function positionTrapViolations(page: import("@playwright/test").Page): Promise<string[]> {
  return page.evaluate((utilities) => {
    const out: string[] = [];
    for (const element of Array.from(document.querySelectorAll(".apx-glass, .apx-glass-strong"))) {
      const classes = Array.from(element.classList);
      const wanted = utilities.find((utility) => classes.includes(utility));
      if (!wanted) continue;
      const actual = getComputedStyle(element).position;
      if (actual !== wanted) {
        const label = element.getAttribute("aria-label") ?? element.getAttribute("data-testid") ?? element.tagName;
        out.push(`${label}: Klasse "${wanted}", errechnet "${actual}"`);
      }
    }
    return out;
  }, POSITION_UTILITIES);
}

test("keine Glasfläche verliert ihre Positionierungs-Utility an die Kaskade", async ({ page }) => {
  await bootWithPhoto(page);

  // Erst der Ruhezustand, dann die drei Zustände, in denen die fünf
  // bekannten Vorfälle tatsächlich auftraten: offenes Überlaufmenü
  // (Menü und Aufklappliste), offenes Entwickeln-Panel (Zoomknöpfe)
  // und laufende Einführung (Scheinwerfer).
  expect(await positionTrapViolations(page), "Ruhezustand").toEqual([]);

  await openOverflowMenu(page);
  await expect(page.getByRole("menuitem").first()).toBeVisible();
  expect(await positionTrapViolations(page), "Überlaufmenü offen").toEqual([]);
  // Das Menue schliesst ueber denselben Knopf; `Escape` laesst die
  // Befehlspalette stehen.
  await page.getByRole("button", { name: "Weitere Funktionen" }).click();
  await expect(page.getByRole("menuitem").first()).toBeHidden();

  await page.locator('[data-tour="develop"]').click();
  await expect(page.getByRole("tab", { name: "Licht" })).toBeVisible();
  expect(await positionTrapViolations(page), "Entwickeln-Panel offen").toEqual([]);
});

/**
 * Nach wie vielen Einträgen eine frische Seite geöffnet wird.
 *
 * Der Test startet die App pro Eintrag neu — anders liesse sich eine
 * Wirkung nicht sauber messen, weil ein geöffneter Dialog das Menü des
 * nächsten Eintrags verdeckt. Siebenundvierzig Neustarts auf
 * **derselben** Seite häufen im Renderer aber so viel an, dass er im
 * Gesamtlauf (zwei Arbeiter parallel) abstürzte: „Target crashed",
 * während derselbe Test allein grün war. Eine neue Seite gibt den
 * Speicher des Renderers wirklich frei; `page.goto` tut das nicht
 * zuverlässig.
 */
const PAGE_RECYCLE_AFTER = 10;

test("jeder Eintrag des Überlaufmenüs wirkt messbar oder ist gesperrt", async ({ page }) => {
  test.setTimeout(300_000);

  await bootWithPhoto(page);
  await openOverflowMenu(page);
  const total = await page.getByRole("menuitem").count();
  // Eine untere Schranke, damit ein versehentlich leeres Menü den Test
  // nicht trivial bestehen lässt.
  expect(total).toBeGreaterThanOrEqual(40);

  const dead: string[] = [];
  const context = page.context();
  let current = page;

  for (let index = 0; index < total; index += 1) {
    if (index > 0 && index % PAGE_RECYCLE_AFTER === 0) {
      const fresh = await context.newPage();
      // Die Fixture-Seite selbst bleibt bestehen; Playwright räumt sie
      // am Testende auf. Geschlossen wird nur, was dieser Test
      // zusätzlich geöffnet hat.
      if (current !== page) await current.close();
      current = fresh;
    }

    // Frischer Start pro Eintrag: ein geöffneter Dialog oder eine
    // gewechselte Ansicht würde sonst das Menü des nächsten Eintrags
    // verdecken und eine Wirkung vortäuschen.
    await bootWithPhoto(current);

    await current.evaluate(() => {
      const internals = (window as unknown as { __TAURI_INTERNALS__: { invoke: (...args: unknown[]) => unknown } })
        .__TAURI_INTERNALS__;
      const log: string[] = [];
      (window as unknown as { __apxInvokeLog: string[] }).__apxInvokeLog = log;
      const original = internals.invoke.bind(internals);
      internals.invoke = (...args: unknown[]) => {
        log.push(String(args[0]));
        return original(...args);
      };
    });

    await openOverflowMenu(current);
    const item = current.getByRole("menuitem").nth(index);
    const label = ((await item.textContent()) ?? "").trim().replace(/\s+/g, " ");

    // Gesperrt ist eine gültige Antwort — der Eintrag sagt dann selbst,
    // dass er gerade nicht anwendbar ist.
    if (await item.isDisabled()) continue;

    const snapshot = () =>
      current.evaluate(() => ({
        dialogs: Array.from(document.querySelectorAll('[role="dialog"]'))
          .map((element) => element.getAttribute("aria-label") ?? "?")
          .sort()
          .join("|"),
        // Vollbild-Überlagerungen (Vergleichsansicht, Karte, Zeitachse)
        // tragen kein `role="dialog"`, sondern sind `fixed` mit Namen.
        overlays: Array.from(document.querySelectorAll("[aria-label]"))
          .filter((element) => getComputedStyle(element).position === "fixed")
          .map((element) => element.getAttribute("aria-label") ?? "?")
          .sort()
          .join("|"),
        tab: document.querySelector('[role="tab"][aria-selected="true"]')?.textContent?.trim() ?? "-",
        main: document.querySelector("main")?.textContent?.slice(0, 200) ?? "-",
        messages: Array.from(document.querySelectorAll('[role="alert"],[role="status"]'))
          .map((element) => element.textContent?.trim().slice(0, 60))
          .sort()
          .join("|"),
        invokes: [...((window as unknown as { __apxInvokeLog?: string[] }).__apxInvokeLog ?? [])],
      }));

    // Grundlinie aus einem gleich langen Ruhefenster: alles, was hier
    // schon von allein über den IPC geht, ist Hintergrundrauschen.
    const idle = await snapshot();
    await current.waitForTimeout(SETTLE_MS);
    const before = await snapshot();
    const noise = new Set([...idle.invokes, ...before.invokes]);

    await item.click();
    await current.waitForTimeout(SETTLE_MS);
    const after = await snapshot();

    const acted =
      after.dialogs !== before.dialogs ||
      after.overlays !== before.overlays ||
      after.tab !== before.tab ||
      after.main !== before.main ||
      after.messages !== before.messages ||
      after.invokes.some((command) => !noise.has(command));

    if (!acted) dead.push(label);
  }

  expect(dead, "Einträge, die klickbar sind und nichts auslösen").toEqual([]);
});
