/**
 * **Effektstufen wirken wirklich** (ADR-0073).
 *
 * Eine Einstellung, die gesetzt wird und nichts ändert, ist schlimmer
 * als keine: sie kostet den Nutzer einen Versuch und lässt ihn mit dem
 * Problem zurück. Dieser Test misst deshalb die ERRECHNETEN
 * Eigenschaften der Glasflächen, nicht das Vorhandensein des Attributs.
 *
 * Gemessen wurde vorher, warum es die Stufen gibt: im Entwickeln-Modus
 * bei 1600×900 tragen acht Glasflächen gleichzeitig `backdrop-filter`
 * UND den SVG-Verzerrungsfilter, zusammen über 1,08 Millionen
 * Bildpunkte.
 */
import { test, expect } from "@playwright/test";
import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000101",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  missing: false,
  camera_model: "Canon EOS",
  content_hash: "hash-effekte-1",
  captured_at: "2024-05-07T12:00:00Z",
};

/** Was auf den Glasflächen tatsächlich gerechnet wird. */
async function glassCost(page: import("@playwright/test").Page) {
  return page.evaluate(() => {
    const surfaces = Array.from(document.querySelectorAll(".apx-glass, .apx-glass-strong"));
    const before = surfaces.map((element) => getComputedStyle(element, "::before"));
    return {
      surfaces: surfaces.length,
      mitVerzerrung: before.filter((style) => style.filter !== "none").length,
      mitWeichzeichner: before.filter(
        (style) =>
          (style.backdropFilter || (style as unknown as Record<string, string>).webkitBackdropFilter) !== "none",
      ).length,
      ballons: document.querySelectorAll(".apx-balloon-layer").length,
      sichtbareBallons: Array.from(document.querySelectorAll<HTMLElement>(".apx-balloon-layer")).filter(
        (element) => getComputedStyle(element).display !== "none",
      ).length,
    };
  });
}

async function boot(page: import("@playwright/test").Page, level: "full" | "reduced" | "plain") {
  await page.setViewportSize({ width: 1600, height: 900 });
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Urlaub", photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
    uiSettings: {
      theme: "birthday" as const,
      accent_color: null,
      locale: "de",
      ui_scale_percent: 100,
      high_contrast: false,
      reduced_motion: false,
      onboarding_seen: true,
      sound_enabled: false,
      sound_volume_percent: 70,
      sound_pack: "cinematic",
      birthday_decorations: true,
      effect_level: level,
    },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
  await page.locator('[data-tour="develop"]').click();
  await expect(page.getByRole("tab", { name: "Licht" })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-effects", level);
}

/**
 * Der Fehler, den dieser Test gefangen hätte (ADR-0076).
 *
 * Die Automatik setzte ihr „schon gemessen"-Flag, BEVOR sie mass. Jede
 * Änderung an den Einstellungen in den 2,5 Sekunden davor räumte den
 * Timer ab, der Effekt lief erneut und stieg sofort wieder aus — die
 * Messung lief nie, und die App blieb auf der teuersten Stufe stehen.
 * Beim Nutzer zeigte die Diagnose daraufhin 83 ms je Bild, also 12
 * Bilder je Sekunde, und Klicks wurden spürbar zu spät angenommen.
 *
 * Geprüft wird deshalb nicht, OB gemessen wurde, sondern was dabei
 * herauskommen muss: bei `auto` darf nie die volle Stufe stehen
 * bleiben.
 */
test("Stufe „automatisch“ lässt nie die teuerste Stufe stehen", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Urlaub", photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
    uiSettings: {
      theme: "birthday" as const,
      accent_color: null,
      locale: "de",
      ui_scale_percent: 100,
      high_contrast: false,
      reduced_motion: false,
      onboarding_seen: true,
      sound_enabled: false,
      sound_volume_percent: 70,
      sound_pack: "cinematic",
      birthday_decorations: true,
      effect_level: "auto" as const,
    },
  });
  await page.goto("/");
  await expect(page.getByRole("button", { name: /Urlaub/ })).toBeVisible();

  const level = await page.locator("html").getAttribute("data-effects");
  expect(level).not.toBe("full");
  expect(["reduced", "plain"]).toContain(level);

  // Und die teuerste Zutat darf dann auch wirklich weg sein — sonst
  // stünde nur ein anderes Wort im Attribut.
  const cost = await glassCost(page);
  expect(cost.mitVerzerrung).toBe(0);
  expect(cost.sichtbareBallons).toBe(0);
});

test("Stufe „voll“: alle Glasflächen tragen Weichzeichner und Verzerrung", async ({ page }) => {
  await boot(page, "full");
  const cost = await glassCost(page);
  // Die Ausgangslage, gegen die die beiden Sparstufen gemessen werden.
  // Schlüge diese Zusicherung fehl, wären die Zusicherungen unten
  // wertlos, weil sie dann nichts mehr ausschlössen.
  expect(cost.surfaces).toBeGreaterThanOrEqual(5);
  expect(cost.mitVerzerrung).toBe(cost.surfaces);
  expect(cost.mitWeichzeichner).toBe(cost.surfaces);
  expect(cost.sichtbareBallons).toBe(1);
});

test("Stufe „reduziert“: keine Verzerrung, keine Dauerbewegung, Weichzeichner bleibt", async ({ page }) => {
  await boot(page, "reduced");
  const cost = await glassCost(page);
  expect(cost.mitVerzerrung).toBe(0);
  // Der Weichzeichner bleibt bewusst: er ist der günstigere der beiden
  // Effekte und trägt den optischen Charakter. Fiele er hier schon weg,
  // gäbe es zwischen „reduziert“ und „schlicht“ keinen Unterschied mehr.
  expect(cost.mitWeichzeichner).toBe(cost.surfaces);
  // Die Ballons sind der Grund, aus dem die Verzerrung überhaupt
  // dauernd neu gerechnet wird — ohne sie bringt das Abschalten der
  // Verzerrung nur einen Bruchteil.
  expect(cost.sichtbareBallons).toBe(0);
});

test("Stufe „schlicht“: kein Hintergrundeffekt, aber lesbarer Grund", async ({ page }) => {
  await boot(page, "plain");
  const cost = await glassCost(page);
  expect(cost.mitVerzerrung).toBe(0);
  expect(cost.mitWeichzeichner).toBe(0);
  expect(cost.sichtbareBallons).toBe(0);

  // Ohne Durchsicht wäre die halbdurchsichtige Glas-Tönung allein zu
  // dünn, um Text davor lesbar zu halten. „Schlicht“ heißt nicht
  // „unlesbar“, deshalb hier die Deckkraft der Tönungsebene.
  const deckkraft = await page.evaluate(() => {
    const surface = document.querySelector(".apx-glass");
    if (!surface) return null;
    const color = getComputedStyle(surface, "::after").backgroundColor;
    const match = /rgba?\(([^)]+)\)/.exec(color);
    if (!match) return null;
    const parts = match[1]!.split(",").map((part) => Number(part.trim()));
    return parts.length < 4 ? 1 : parts[3]!;
  });
  expect(deckkraft).not.toBeNull();
  expect(deckkraft!).toBeGreaterThanOrEqual(0.9);
});
