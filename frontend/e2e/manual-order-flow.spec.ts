import { expect, test } from "@playwright/test";

import { installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

const base = { rating: 0, flag: 0, color_label: null, missing: false, width: 6000, height: 4000 };
const PHOTOS = [
  { ...base, id: "01977f4a-0000-7000-8000-000000000101", filename: "A.CR3" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000102", filename: "B.CR3" },
  { ...base, id: "01977f4a-0000-7000-8000-000000000103", filename: "C.CR3" },
];

/**
 * Deckt Phase 33 F8 ab: die manuelle Reihenfolge in einer Sammlung. Die
 * eigentliche Umordnung (Off-by-one beim Zielindex, lückenlose
 * Positionen, Ablehnung bei intelligenten Sammlungen) ist in
 * `apx-catalog`s `collections`-Tests abgedeckt — hier geht es darum, dass
 * das Ziehen überhaupt nur dort angeboten wird, wo es hält, was es
 * verspricht.
 */
test.describe("Manuelle Reihenfolge (Phase 33 F8)", () => {
  async function openCollection(page: import("@playwright/test").Page) {
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();
    // Alle drei in eine neue Sammlung legen.
    await page.getByRole("img", { name: "A.CR3" }).first().click();
    await page.getByRole("img", { name: "B.CR3" }).first().click({ modifiers: ["Control"] });
    await page.getByRole("img", { name: "C.CR3" }).first().click({ modifiers: ["Control"] });
  }

  test("ohne manuelle Sortierung sind die Kacheln nicht ziehbar", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await openCollection(page);

    // Im Ordner (keine Sammlung, Sortierung nach Dateiname) darf nichts
    // ziehbar sein — sonst wäre das Ziehen ein Versprechen, das die
    // Ansicht im nächsten Moment bricht.
    const cell = page.getByRole("button", { name: /A\.CR3/ }).first();
    await expect(cell).toHaveAttribute("draggable", "false");
  });

  test("die Sortierauswahl kennt die manuelle Reihenfolge", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("button", { name: "Raster" }).click();

    const sort = page.getByLabel("Sortieren nach");
    await expect(sort).toContainText("Manuell (Sammlung)");
    await sort.selectOption("manual");
    // Ohne Sammlung bleibt die Reihenfolge, wie sie kommt — und die
    // Kacheln bleiben unziehbar.
    await expect(page.getByRole("button", { name: /A\.CR3/ }).first()).toHaveAttribute("draggable", "false");
  });

  test("in einer Sammlung mit manueller Sortierung ordnet Ziehen wirklich um", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: PHOTOS.length }],
      photosByFolder: { [FOLDER_ID]: PHOTOS },
    });
    await openCollection(page);

    await page.getByRole("button", { name: "Neue Sammlung" }).click();
    await page.getByPlaceholder("Name der Sammlung…").fill("Auswahl");
    await page.getByPlaceholder("Name der Sammlung…").press("Enter");
    await page.getByRole("button", { name: "Auswahl zu dieser Sammlung hinzufügen" }).click();
    await page.getByRole("button", { name: "Auswahl", exact: true }).click();

    await page.getByLabel("Sortieren nach").selectOption("manual");

    const cellC = page.getByRole("button", { name: /C\.CR3/ }).first();
    await expect(cellC).toHaveAttribute("draggable", "true");

    // C an den Anfang ziehen.
    await cellC.dragTo(page.getByRole("button", { name: /A\.CR3/ }).first());

    const names = await page.getByTestId("photo-grid").getByRole("img").evaluateAll((nodes) =>
      nodes.map((node) => (node as HTMLImageElement).alt),
    );
    expect(names.slice(0, 3)).toEqual(["C.CR3", "A.CR3", "B.CR3"]);
  });
});
