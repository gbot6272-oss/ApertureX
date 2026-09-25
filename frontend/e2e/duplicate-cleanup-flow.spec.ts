import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";

/**
 * Deckt Phase 34 F9 ab (`DECISIONS.md` ADR-0070): den Duplikat-
 * Assistenten. Die Auswahlregel selbst ist in `apx-app`s
 * `duplicate_keeper`-Unit-Tests abgedeckt — hier geht es um das, was
 * nur die Oberfläche halten kann: dass der Vorschlag begründet ist,
 * dass er sich umstellen lässt, und dass danach wirklich etwas im
 * Papierkorb landet.
 */
test.describe("Duplikat-Assistent (Phase 34 F9)", () => {
  const RAW = {
    id: "01977f4a-0000-7000-8000-000000000101",
    filename: "IMG_0001.CR3",
    width: 6000,
    height: 4000,
    file_size: 30_000_000,
    missing: false,
    content_hash: "abc123",
  };
  const JPEG = {
    id: "01977f4a-0000-7000-8000-000000000102",
    filename: "IMG_0001.JPG",
    width: 6000,
    height: 4000,
    file_size: 40_000_000,
    missing: false,
    content_hash: "abc123",
  };

  test("begründet den Vorschlag und legt die übrigen Versionen in den Papierkorb", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: [JPEG, RAW] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();

    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: /Duplikate aufräumen/ }).click();
    const dialog = page.getByRole("dialog", { name: "Duplikate aufräumen" });

    await expect(dialog.getByTestId("duplicate-cleanup-apply")).toBeDisabled();
    await dialog.getByTestId("duplicate-cleanup-scan").click();

    // Das RAW gewinnt gegen das GRÖSSERE JPEG — und der Dialog sagt warum.
    await expect(dialog.getByTestId("duplicate-cleanup-list")).toContainText("ist das RAW, nicht der Ableger");
    await expect(dialog.getByTestId("duplicate-cleanup-summary")).toContainText("1 würden in den Papierkorb");

    await dialog.getByTestId("duplicate-cleanup-apply").click();
    await expect(dialog.getByTestId("duplicate-cleanup-result")).toContainText("1 Foto in den Papierkorb");
  });

  test("lässt den Vorschlag umstellen — dann geht das andere Foto weg", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: [JPEG, RAW] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();

    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: /Duplikate aufräumen/ }).click();
    const dialog = page.getByRole("dialog", { name: "Duplikate aufräumen" });
    await dialog.getByTestId("duplicate-cleanup-scan").click();

    await dialog.getByLabel("IMG_0001.JPG behalten").check();
    await dialog.getByTestId("duplicate-cleanup-apply").click();
    await expect(dialog.getByTestId("duplicate-cleanup-result")).toContainText("1 Foto in den Papierkorb");

    // Nach dem Aufräumen ist die Gruppe weg: nur noch eine Version da.
    await expect(dialog.getByTestId("duplicate-cleanup-list")).toContainText("Keine Duplikatgruppen gefunden");
    await expect(page.getByRole("img", { name: "IMG_0001.JPG" })).toBeVisible();
  });

  test("überspringt eine Gruppe vollständig, wenn man sie abwählt", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 2 }],
      photosByFolder: { [FOLDER_ID]: [JPEG, RAW] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();

    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: /Duplikate aufräumen/ }).click();
    const dialog = page.getByRole("dialog", { name: "Duplikate aufräumen" });
    await dialog.getByTestId("duplicate-cleanup-scan").click();

    await dialog.getByLabel("Gruppe 1 überspringen").check();
    await expect(dialog.getByTestId("duplicate-cleanup-summary")).toContainText("0 würden in den Papierkorb");
    await expect(dialog.getByTestId("duplicate-cleanup-apply")).toBeDisabled();
  });
});
