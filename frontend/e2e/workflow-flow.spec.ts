import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

/**
 * Deckt Phase 6 Schritt 8 ab (`DECISIONS.md` ADR-0032, `SPEC.md` §3.4):
 * Schnappschüsse (benannte EDL-Zwischenstände zusätzlich zum linearen
 * Verlauf) und Vorher/Nachher (vier Ansichten im Viewer).
 */
const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000101",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  camera_make: "Canon",
  camera_model: "EOS R5",
  lens: "RF 24-70mm",
  iso: 200,
  aperture: 4,
  shutter: 1 / 250,
  focal_length: 50,
  captured_at: "2024-06-01T10:00:00Z",
  missing: false,
};
const PHOTO_2 = { ...PHOTO, id: "01977f4a-0000-7000-8000-000000000102", filename: "IMG_0002.CR3" };

async function setUpWithSelectedPhoto(page: import("@playwright/test").Page, extraPhotos: (typeof PHOTO)[] = []) {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 + extraPhotos.length }],
    photosByFolder: { [FOLDER_ID]: [PHOTO, ...extraPhotos] },
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
  await page.getByRole("button", { name: "Entwickeln" }).click();
}

async function lastCommittedExposure(page: import("@playwright/test").Page): Promise<number> {
  const log = await getMockInvokeLog(page);
  const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
  return JSON.parse((lastCommit?.args as { edlJson: string }).edlJson).payload.basic.exposure_ev;
}

async function lastExposureFor(page: import("@playwright/test").Page, photoId: string): Promise<number | undefined> {
  const log = await getMockInvokeLog(page);
  const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit" && (entry.args as { photoId: string }).photoId === photoId);
  if (!lastCommit) return undefined;
  return JSON.parse((lastCommit.args as { edlJson: string }).edlJson).payload.basic.exposure_ev;
}

test.describe("Workflow: Schnappschüsse + Vorher/Nachher", () => {
  test("Schnappschuss speichern und wiederherstellen committet jeweils, Umbenennen und Löschen auch", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" }).first();
    await exposureInput.fill("0.8");
    await exposureInput.blur();
    await expect.poll(async () => lastCommittedExposure(page)).toBeCloseTo(0.8, 2);

    page.once("dialog", (dialog) => void dialog.accept("Erste Version"));
    await page.getByRole("button", { name: "+ Schnappschuss vom aktuellen Stand" }).click();
    await expect(page.getByRole("button", { name: "Erste Version", exact: true })).toBeVisible();

    // Weiter bearbeiten, dann den Schnappschuss wiederherstellen.
    await exposureInput.fill("-1.2");
    await exposureInput.blur();
    await expect.poll(async () => lastCommittedExposure(page)).toBeCloseTo(-1.2, 2);

    await page.getByRole("button", { name: "Erste Version", exact: true }).click();
    await expect.poll(async () => lastCommittedExposure(page)).toBeCloseTo(0.8, 2);
    await expect(exposureInput).toHaveValue("0.8");

    page.once("dialog", (dialog) => void dialog.accept("Referenz"));
    await page.getByTitle("Umbenennen").click();
    await expect(page.getByRole("button", { name: "Referenz", exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Schnappschuss Referenz löschen" }).click();
    await expect(page.getByText("Noch keine Schnappschüsse.")).toBeVisible();
  });

  test("Vorher/Nachher-Modi schalten die Ansicht um, jeweils nur ein Modus aktiv", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    const sideBySideButton = page.getByRole("button", { name: "Links/Rechts" });
    const stackedButton = page.getByRole("button", { name: "Oben/Unten" });
    const splitButton = page.getByRole("button", { name: "Geteilt", exact: true });

    await expect(page.getByLabel("Vorher/Nachher")).toHaveCount(0);

    await sideBySideButton.click();
    await expect(sideBySideButton).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByLabel("Vorher/Nachher")).toBeVisible();
    await expect(page.getByText("Vorher", { exact: true })).toBeVisible();
    await expect(page.getByText("Nachher", { exact: true })).toBeVisible();

    // Ein anderer Modus übernimmt, der vorherige wird deaktiviert.
    await stackedButton.click();
    await expect(stackedButton).toHaveAttribute("aria-pressed", "true");
    await expect(sideBySideButton).toHaveAttribute("aria-pressed", "false");
    await expect(page.getByLabel("Vorher/Nachher")).toBeVisible();

    // Erneutes Klicken auf den aktiven Modus schaltet zurück auf "keine Ansicht".
    await stackedButton.click();
    await expect(stackedButton).toHaveAttribute("aria-pressed", "false");
    await expect(page.getByLabel("Vorher/Nachher")).toHaveCount(0);

    await splitButton.click();
    await expect(splitButton).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByLabel("Vorher/Nachher")).toBeVisible();
  });
});

/**
 * Deckt Phase 6 Schritt 9 ab (`DECISIONS.md` ADR-0032, `SPEC.md` §3.4):
 * Einstellungen kopieren/einfügen (Teilmenge über Sektions-Checkboxen,
 * dasselbe `PresetEdlSubset`-Mechanismus wie Phase 5s Presets), "Vorherige
 * übernehmen" (übernimmt den zuletzt im Entwickeln-Modul offenen Foto-
 * Stand), Synchronisieren auf die übrige Mehrfachauswahl und Auto-Sync.
 */
test.describe("Workflow: Kopieren/Einfügen + Vorherige + Synchronisieren", () => {
  test("Kopieren merkt sich die markierten Sektionen, Einfügen wendet sie an und committet", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" }).first();
    await exposureInput.fill("0.8");
    await exposureInput.blur();
    await expect.poll(async () => lastCommittedExposure(page)).toBeCloseTo(0.8, 2);

    // Alle Sektionen sind per Vorgabe markiert (siehe `DevelopPanel.tsx`s
    // `workflowSections`-Initialwert) — Kopieren übernimmt also auch die
    // Belichtung.
    await page.getByRole("button", { name: "Kopieren" }).click();

    await exposureInput.fill("-1.2");
    await exposureInput.blur();
    await expect.poll(async () => lastCommittedExposure(page)).toBeCloseTo(-1.2, 2);

    await page.getByRole("button", { name: "Einfügen" }).click();
    await expect.poll(async () => lastCommittedExposure(page)).toBeCloseTo(0.8, 2);
    await expect(exposureInput).toHaveValue("0.8");
  });

  test("Vorherige übernehmen kopiert den zuletzt offenen Foto-Stand auf das aktuelle Foto und committet", async ({ page }) => {
    await setUpWithSelectedPhoto(page, [PHOTO_2]);

    // "Vorherige übernehmen" ist deaktiviert, solange noch kein anderes
    // Foto im Entwickeln-Modul offen war.
    const applyPreviousButton = page.getByRole("button", { name: "Vorherige übernehmen" });
    await expect(applyPreviousButton).toBeDisabled();

    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" }).first();
    await exposureInput.fill("0.6");
    await exposureInput.blur();
    await expect.poll(async () => lastExposureFor(page, PHOTO.id)).toBeCloseTo(0.6, 2);

    // Zum zweiten Foto wechseln — das Entwickeln-Panel bleibt offen und
    // lädt automatisch dessen (neutralen) Stand; `lastDevelopPhotoId`
    // merkt sich dabei das erste Foto.
    await page.getByRole("img", { name: PHOTO_2.filename }).click();
    await expect(applyPreviousButton).toBeEnabled();

    await applyPreviousButton.click();
    await expect.poll(async () => lastExposureFor(page, PHOTO_2.id)).toBeCloseTo(0.6, 2);
    await expect(exposureInput).toHaveValue("0.6");
  });

  test("Synchronisieren überträgt die markierten Sektionen auf die übrige Mehrfachauswahl", async ({ page }) => {
    await setUpWithSelectedPhoto(page, [PHOTO_2]);

    // Zusätzlich zum bereits ausgewählten ersten Foto das zweite per
    // Strg-Klick zur Mehrfachauswahl hinzufügen (`resolveSelectionMode`:
    // `ctrlKey`/`metaKey` → "toggle").
    await page.getByRole("img", { name: PHOTO_2.filename }).click({ modifiers: ["Control"] });

    const syncButton = page.getByRole("button", { name: "Auf 1 weitere ausgewählte Foto synchronisieren" });
    await expect(syncButton).toBeEnabled();

    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" }).first();
    await exposureInput.fill("0.5");
    await exposureInput.blur();
    await expect.poll(async () => lastExposureFor(page, PHOTO_2.id)).toBeCloseTo(0.5, 2);

    await syncButton.click();
    await expect.poll(async () => lastExposureFor(page, PHOTO.id)).toBeCloseTo(0.5, 2);
  });

  test("Auto-Sync überträgt jede Änderung sofort auf die übrige Mehrfachauswahl, ohne den Sync-Knopf zu klicken", async ({ page }) => {
    await setUpWithSelectedPhoto(page, [PHOTO_2]);
    await page.getByRole("img", { name: PHOTO_2.filename }).click({ modifiers: ["Control"] });

    await page.getByRole("checkbox", { name: /Auto-Sync/ }).check();

    const exposureInput = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" }).first();
    await exposureInput.fill("-0.3");
    await exposureInput.blur();

    await expect.poll(async () => lastExposureFor(page, PHOTO_2.id)).toBeCloseTo(-0.3, 2);
    // Kein Klick auf den Sync-Knopf nötig — Auto-Sync löst die
    // Übertragung direkt aus `commitDevelopEdit` heraus aus.
    await expect.poll(async () => lastExposureFor(page, PHOTO.id)).toBeCloseTo(-0.3, 2);
  });
});

/**
 * Deckt Phase 6 Schritt 10 ab (`DECISIONS.md` ADR-0032, `SPEC.md`
 * §3.4/§7): Referenzansicht (Referenzbild links, Arbeitsbild rechts) und
 * Soft-Proof (Zielprofil, Renderpriorität, Farbumfangswarnung,
 * Papierweiß-Simulation) — beides reine Viewer-/Anzeige-Zustände, ohne
 * Backend-Commit (siehe `lib/softProof.ts`s Moduldoku).
 */
test.describe("Workflow: Referenzansicht + Soft-Proof", () => {
  test("Referenzansicht zeigt Referenz- und Arbeitsbild nebeneinander an und lässt sich wieder ausblenden", async ({ page }) => {
    await setUpWithSelectedPhoto(page, [PHOTO_2]);

    await expect(page.getByLabel("Referenzansicht")).toHaveCount(0);

    const showButton = page.getByRole("button", { name: "Referenzansicht anzeigen" });
    // Ohne gewähltes Referenzfoto bleibt der Knopf deaktiviert.
    await expect(showButton).toBeDisabled();

    await page.getByRole("combobox", { name: "Referenzfoto" }).selectOption(PHOTO_2.id);
    await expect(showButton).toBeEnabled();
    await showButton.click();

    const referenceView = page.getByLabel("Referenzansicht");
    await expect(referenceView).toBeVisible();
    await expect(referenceView.getByText("Referenz", { exact: true })).toBeVisible();
    await expect(referenceView.getByText("Arbeitsbild", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Referenzansicht ausblenden" }).click();
    await expect(page.getByLabel("Referenzansicht")).toHaveCount(0);
  });

  test("Soft-Proof schaltet Zielprofil/Renderpriorität/Warnung/Papierweiß frei, ohne einen Commit auszulösen", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    await expect(page.getByLabel("Soft-Proof-Zielprofil")).toHaveCount(0);

    const logBefore = await getMockInvokeLog(page);
    const commitsBefore = logBefore.filter((entry) => entry.cmd === "apply_develop_edit").length;

    await page.getByRole("button", { name: "Soft-Proof einschalten" }).click();
    const profileSelect = page.getByRole("combobox", { name: "Soft-Proof-Zielprofil" });
    const intentSelect = page.getByRole("combobox", { name: "Soft-Proof-Renderpriorität" });
    await expect(profileSelect).toBeVisible();
    await expect(intentSelect).toBeVisible();

    await profileSelect.selectOption("print_sim");
    await intentSelect.selectOption("relative_colorimetric");
    await page.getByLabel("Farbumfangswarnung").check();
    await page.getByLabel("Papierweiß-Simulation").check();

    await expect(profileSelect).toHaveValue("print_sim");
    await expect(intentSelect).toHaveValue("relative_colorimetric");
    await expect(page.getByLabel("Farbumfangswarnung")).toBeChecked();
    await expect(page.getByLabel("Papierweiß-Simulation")).toBeChecked();

    // Soft-Proof ist reine Anzeige-Nachbearbeitung des Vorschau-Puffers
    // (siehe `lib/softProof.ts`) — keiner der obigen Klicks committet.
    const logAfter = await getMockInvokeLog(page);
    const commitsAfter = logAfter.filter((entry) => entry.cmd === "apply_develop_edit").length;
    expect(commitsAfter).toBe(commitsBefore);

    await page.getByRole("button", { name: "Soft-Proof ausschalten" }).click();
    await expect(page.getByLabel("Soft-Proof-Zielprofil")).toHaveCount(0);
  });
});
