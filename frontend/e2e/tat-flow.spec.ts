import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = { id: "01977f4a-0000-7000-8000-000000000101", filename: "IMG_0001.CR3", width: 6000, height: 4000, missing: false };

/**
 * Deckt Phase 11 Schritt 6 ab (`PLAN.md`, `DECISIONS.md` ADR-0038):
 * zielgerichtetes Anpassungswerkzeug (TAT) im Kurven-Modus — Klick+Zug
 * im Bild legt einen Kurvenpunkt an (oder verschiebt den nächst-
 * gelegenen) und committet über denselben `apply_develop_edit`-Pfad wie
 * die übrigen Regler. Der HSL-Modus teilt sich denselben Zug-Code-Pfad
 * in `Viewer.tsx` (nur `setHslBandField` statt `setCurveChannel`) und
 * wird hier bewusst nicht separat getestet — siehe die vom Nutzer
 * angeordnete gelockerte Testdisziplin (max. 1 Test je Schritt, siehe
 * ADR-0038-Nachtrag).
 */
test.describe("Zielgerichtetes Anpassungswerkzeug (Phase 11 Schritt 6)", () => {
  test("TAT-Kurvenmodus legt beim Ziehen nach oben einen Kurvenpunkt mit höherem Ausgabewert an", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await page.getByRole("button", { name: "Entwickeln" }).click();

    const tatGroup = page.getByRole("group", { name: "Zielgerichtetes Anpassungswerkzeug" });
    const tatCurveButton = tatGroup.getByRole("button", { name: "TAT: Kurve" });
    await expect(tatCurveButton).toHaveAttribute("aria-pressed", "false");
    await tatCurveButton.click();
    await expect(tatCurveButton).toHaveAttribute("aria-pressed", "true");

    const mainBox = await page.getByRole("main").boundingBox();
    if (!mainBox) throw new Error("Viewer-Container nicht gefunden");
    const centerX = mainBox.x + mainBox.width / 2;
    const centerY = mainBox.y + mainBox.height / 2;

    // Nach oben ziehen (Lightroom-Konvention, siehe Viewer.tsx-Moduldoku):
    // erhöht den Ausgabewert am gesampelten Kurvenpunkt.
    await page.mouse.move(centerX, centerY);
    await page.mouse.down();
    await page.mouse.move(centerX, centerY - 80, { steps: 4 });
    await page.mouse.up();

    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        return log.some((entry) => entry.cmd === "apply_develop_edit");
      })
      .toBe(true);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const rgbCurve = JSON.parse(edlJson).payload.curves.rgb as { kind: string; points: Array<{ input: number; output: number }> };
    expect(rgbCurve.kind).toBe("Points");
    // Mindestens ein Punkt muss über die Identität hinaus nach oben
    // verschoben worden sein (output > input) — die Kern-Behauptung des
    // TAT-Kurvenmodus.
    expect(rgbCurve.points.some((p) => p.output > p.input + 0.01)).toBe(true);
  });
});
