import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000701";
const base = { width: 6000, height: 4000, missing: false };
const REFERENZ = { ...base, id: "01977f4a-0000-7000-8000-000000000702", filename: "REF.CR3" };
const DUNKEL = { ...base, id: "01977f4a-0000-7000-8000-000000000703", filename: "DUNKEL.CR3" };
const HELL = { ...base, id: "01977f4a-0000-7000-8000-000000000704", filename: "HELL.CR3" };

/**
 * Phase 34 F2 (siehe `DECISIONS.md` ADR-0070). Die Mathematik selbst
 * liegt in `exposure_match.rs`s neun Rust-Tests — hier laeuft die Kette
 * dazwischen: Referenz waehlen, messen, anwenden, und landet die
 * Korrektur wirklich im committeten EDL.
 *
 * Die Vorschau-Helligkeiten kommen aus den Fixtures: `DUNKEL` ist halb
 * so hell wie die Referenz (= +1 EV noetig), `HELL` doppelt so hell
 * (= −1 EV).
 */
test.describe("Belichtung angleichen (Phase 34 F2)", () => {
  test("misst die Abweichung und schreibt sie relativ ins EDL", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Serie", photo_count: 3, parent_id: null, missing: false }],
      photosByFolder: { [FOLDER_ID]: [REFERENZ, DUNKEL, HELL] },
      exposureLevels: { [REFERENZ.id]: 0.4, [DUNKEL.id]: 0.2, [HELL.id]: 0.8 },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Serie/ }).click();

    // Alle drei auswaehlen; das zuletzt angeklickte Foto ist die Referenz.
    await page.getByRole("img", { name: DUNKEL.filename }).click();
    await page.getByRole("img", { name: HELL.filename }).click({ modifiers: ["Control"] });
    await page.getByRole("img", { name: REFERENZ.filename }).click({ modifiers: ["Control"] });

    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: "Belichtung angleichen…" }).click();

    await expect(page.getByTestId("exposure-match-summary")).toContainText("2 von 2");
    const list = page.getByTestId("exposure-match-list");
    await expect(list).toContainText("+1.00 EV");
    await expect(list).toContainText("-1.00 EV");

    await page.getByTestId("exposure-match-apply").click();
    await expect(page.getByRole("status")).toContainText("2 Fotos angeglichen");

    // Der eigentliche Beleg: was wurde committet?
    const log = await getMockInvokeLog(page);
    const commits = log.filter((entry) => entry.cmd === "apply_develop_edit");
    expect(commits).toHaveLength(2);
    const byPhoto = new Map(
      commits.map((entry) => {
        const args = entry.args as { photoId: string; edlJson: string };
        return [args.photoId, JSON.parse(args.edlJson).payload.basic.exposure_ev as number];
      }),
    );
    expect(byPhoto.get(DUNKEL.id)).toBeCloseTo(1, 5);
    expect(byPhoto.get(HELL.id)).toBeCloseTo(-1, 5);
    // Die Referenz selbst bleibt unangetastet.
    expect(byPhoto.has(REFERENZ.id)).toBe(false);
  });
});
