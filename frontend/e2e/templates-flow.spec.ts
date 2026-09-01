import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };
const PRESET_ID = "01977f4a-0000-7000-8000-000000000201";

/**
 * Deckt Phase 8 Schritt 8 ab (`PLAN.md`, `apx_catalog::Template`s
 * Moduldoku): den Vorlagen-Dialog. Die generische CRUD/Datei-Logik ist
 * bereits vollständig in `apx-catalog`s Rust-Unit-Tests (`templates.rs`)
 * abgedeckt — hier bewusst nur die drei wichtigsten Frontend-Fälle.
 */
test.describe("Vorlagen (Phase 8 Schritt 8)", () => {
  test("Vorlagen-Knopf öffnet den Dialog, eine neue Export-Vorlage erscheint in der Liste", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: "Vorlagen…" }).click();
    await page.getByLabel("Art").selectOption("export");
    await expect(page.getByText("Keine gespeicherten Vorlagen")).toBeVisible();

    await page.getByLabel("Name").fill("Web-Export Standard");
    await page.getByRole("button", { name: "Vorlage speichern" }).click();

    await expect(page.getByText("Web-Export Standard")).toBeVisible();
    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "save_template");
    expect(call).toBeDefined();
    expect((call?.args as { kind: string }).kind).toBe("export");
  });

  test("Workflow-Vorlage anlegen und ausführen wendet das Preset auf alle Fotos an und exportiert sie", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
      presets: [{ id: PRESET_ID, folder_id: null, name: "Filmlook", is_favorite: false, tags: [], conditions_json: "[]", created_at: new Date().toISOString() }],
      presetVersions: { [PRESET_ID]: [{ id: "v1", preset_id: PRESET_ID, sequence: 1, edl_subset_json: JSON.stringify({ basic: { exposure_ev: 0.5 } }), created_at: new Date().toISOString() }] },
      selectFolderResult: "/home/user/Export/Urlaub",
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Vorlagen…" }).click();
    await page.getByLabel("Art").selectOption("workflow");

    await page.getByLabel("Name").fill("Urlaub-Export");
    await page.getByLabel("Preset").selectOption(PRESET_ID);
    await page.getByRole("button", { name: "Workflow-Vorlage speichern" }).click();
    await expect(page.getByText("Urlaub-Export")).toBeVisible();

    await page.getByRole("button", { name: "Ausführen" }).click();
    await expect(page.getByText("Workflow: 1 / 1 verarbeitet")).toBeVisible();

    const log = await getMockInvokeLog(page);
    expect(log.some((e) => e.cmd === "apply_develop_edit")).toBe(true);
    expect(log.some((e) => e.cmd === "export_photo")).toBe(true);
  });
});
