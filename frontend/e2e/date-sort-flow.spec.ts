import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Speicherkarte";

/**
 * Deckt Phase 34 F8 ab (`DECISIONS.md` ADR-0070): das nachträgliche
 * Einsortieren nach Aufnahmedatum. Die Plan-Logik selbst ist in
 * `apx-app`s `date_sort`-Unit-Tests abgedeckt — hier geht es um die
 * beiden Zusagen, die nur die Oberfläche halten kann: dass ohne
 * Vorschau nichts verschoben wird, und dass ein Namenskonflikt sichtbar
 * gemeldet statt stillschweigend überschrieben wird.
 */
test.describe("Nach Aufnahmedatum einsortieren (Phase 34 F8)", () => {
  test("zeigt erst den Plan und verschiebt die Fotos dann in ihre Datumsordner", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: {
        [FOLDER_ID]: [
          {
            id: "01977f4a-0000-7000-8000-000000000101",
            filename: "IMG_0001.CR3",
            width: 6000,
            height: 4000,
            missing: false,
            captured_at: "2024-05-07T12:00:00Z",
          },
          {
            id: "01977f4a-0000-7000-8000-000000000102",
            filename: "IMG_0002.CR3",
            width: 6000,
            height: 4000,
            missing: false,
            captured_at: "2024-06-18T09:30:00Z",
          },
        ],
      },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Speicherkarte/ }).click();

    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: /Nach Aufnahmedatum einsortieren/ }).click();

    const dialog = page.getByRole("dialog", { name: "Nach Aufnahmedatum einsortieren" });
    await expect(dialog).toBeVisible();

    // Ohne Vorschau ist „Einsortieren" gesperrt — es wird nichts
    // verschoben, was niemand gesehen hat.
    await expect(dialog.getByTestId("date-sort-apply")).toBeDisabled();

    await dialog.getByTestId("date-sort-preview").click();
    await expect(dialog.getByTestId("date-sort-summary")).toContainText("2 zu verschieben");
    await expect(dialog.getByTestId("date-sort-list")).toContainText("2024/2024-05-07");
    await expect(dialog.getByTestId("date-sort-list")).toContainText("2024/2024-06-18");

    await dialog.getByTestId("date-sort-apply").click();
    await expect(dialog.getByTestId("date-sort-result")).toContainText("2 Fotos einsortiert");
    // Die zweite Vorschau läuft automatisch: danach liegt alles richtig.
    await expect(dialog.getByTestId("date-sort-summary")).toContainText("0 zu verschieben");
  });

  test("meldet zwei gleichnamige Fotos desselben Tages als Namenskonflikt statt eines zu überschreiben", async ({ page }) => {
    const OTHER_FOLDER_ID = "01977f4a-0000-7000-8000-000000000002";
    await installTauriMock(page, {
      folders: [
        { id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 },
        { id: OTHER_FOLDER_ID, path: "/home/user/Fotos/SpeicherkarteB", photo_count: 1 },
      ],
      photosByFolder: {
        [FOLDER_ID]: [
          {
            id: "01977f4a-0000-7000-8000-000000000101",
            filename: "IMG_0001.CR3",
            width: 6000,
            height: 4000,
            missing: false,
            captured_at: "2024-05-07T12:00:00Z",
          },
        ],
        [OTHER_FOLDER_ID]: [
          {
            id: "01977f4a-0000-7000-8000-000000000201",
            filename: "IMG_0001.CR3",
            width: 6000,
            height: 4000,
            missing: false,
            captured_at: "2024-05-07T15:00:00Z",
          },
        ],
      },
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Speicherkarte 1" }).click();

    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: /Nach Aufnahmedatum einsortieren/ }).click();
    const dialog = page.getByRole("dialog", { name: "Nach Aufnahmedatum einsortieren" });

    // Der ganze Katalog, damit beide Ordner im Plan stehen.
    await dialog.getByLabel("Den ganzen Katalog einsortieren").check();
    await dialog.getByLabel("Zielordner").fill("/home/user/Fotos");
    await dialog.getByTestId("date-sort-preview").click();

    await expect(dialog.getByTestId("date-sort-summary")).toContainText("1 Namenskonflikte");
    await expect(dialog.getByTestId("date-sort-list")).toContainText("Namenskonflikt");
  });
});
