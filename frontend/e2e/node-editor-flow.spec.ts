import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 9 Schritt 7 ab (`PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 1):
 * den Node-Editor im Entwickeln-Panel. Die eigentliche Stufen-
 * Überspringen-Logik (`develop::render_rgba8`) ist bereits vollständig in
 * `apx-pipeline`s Rust-Unit-Tests abgedeckt — hier bewusst nur ein
 * Frontend-Flow: eine Stufe deaktivieren committet `stage_enabled` korrekt,
 * andere Stufen bleiben unberührt.
 */
test.describe("Node-Editor (Phase 9 Schritt 7)", () => {
  test("eine Stufe deaktivieren committet stage_enabled, andere Stufen bleiben aktiv", async ({ page }) => {
    await installTauriMock(page, { folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }], photosByFolder: { [FOLDER_ID]: [PHOTO] } });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const nodeEditor = page.getByRole("group", { name: "Node-Editor" });
    await expect(nodeEditor.getByText("Grundeinstellungen")).toBeVisible();
    await nodeEditor.getByRole("checkbox", { name: "Details aktiv" }).uncheck();

    await expect.poll(async () => {
      const log = await getMockInvokeLog(page);
      const last = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
      const edlJson = (last?.args as { edlJson?: string } | undefined)?.edlJson;
      if (!edlJson) return null;
      const stageEnabled = JSON.parse(edlJson).payload.stage_enabled as Record<string, boolean>;
      return stageEnabled.details;
    }).toBe(false);

    const log = await getMockInvokeLog(page);
    const last = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (last?.args as { edlJson?: string } | undefined)?.edlJson as string;
    const stageEnabled = JSON.parse(edlJson).payload.stage_enabled as Record<string, boolean>;
    expect(stageEnabled.basic).toBe(true);
    expect(stageEnabled.curves).toBe(true);

    // "Öffnen" springt zum zugehörigen Regler-Abschnitt statt zu
    // navigieren — muss ohne Fehler funktionieren, auch wenn der Anker
    // (bei "Details") bereits sichtbar ist.
    await nodeEditor.getByRole("listitem").filter({ hasText: "Details" }).getByRole("button", { name: "Öffnen" }).click();
    await expect(page.getByRole("group", { name: "Details" })).toBeVisible();
  });
});
