import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000101",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  missing: false,
};

async function setUpWithSelectedPhoto(page: import("@playwright/test").Page, extraFixtures: Record<string, unknown> = {}) {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
    ...extraFixtures,
  });
  await page.goto("/");
  await page.getByRole("button", { name: /Urlaub/ }).click();
  await page.getByRole("img", { name: PHOTO.filename }).click();
}

/**
 * Deckt Phase 8 Schritt 4 ab (`PLAN.md`/`DECISIONS.md` ADR-0034): den
 * Diashow-Dialog, den Aufruf der Live-Wiedergabe (`SlideshowPlayer.tsx`)
 * sowie den optionalen Video-Export über ein simuliertes System-`ffmpeg`.
 * Die eigentliche Ken-Burns-/Zeitachsen-Mathematik ist bereits vollständig
 * in `lib/slideshow.test.ts` (Vitest) abgedeckt, die Frame-/Übergangs-
 * Komposition des Video-Exports in `apx-export`s Rust-Unit-Tests
 * (`video.rs`) — hier nur die Frontend-Verdrahtung, auf die
 * verdrahtungsrelevanten Fälle eingedampft (Knopf-Aktivierung, ffmpeg-
 * Verfügbarkeit, Live-Wiedergabe, Dialog-Abbruch, Export-Erfolg mit
 * Optionen, ein bedingter Zweig (Intro-Schriftdatei), Fehlerfall).
 */
test.describe("Diashow (Phase 8 Schritt 4)", () => {
  test("Diashow-Knopf ist ohne Auswahl deaktiviert, mit Auswahl geht der Dialog auf", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Diashow…" })).toBeDisabled();

    await page.getByRole("button", { name: /Urlaub/ }).click();
    await page.getByRole("img", { name: PHOTO.filename }).click();
    await expect(page.getByRole("button", { name: "Diashow…" })).toBeEnabled();
    await page.getByRole("button", { name: "Diashow…" }).click();
    await expect(page.getByText("1 Foto", { exact: true })).toBeVisible();
  });

  test("fehlendes ffmpeg deaktiviert den Video-Export-Knopf und zeigt einen Hinweis", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { ffmpegAvailable: false });
    await page.getByRole("button", { name: "Diashow…" }).click();
    await expect(page.getByText(/ffmpeg wurde auf diesem Rechner nicht gefunden/)).toBeVisible();
    await expect(page.getByRole("button", { name: "Als Video exportieren (MP4)" })).toBeDisabled();
  });

  test("Abspielen öffnet die Live-Wiedergabe mit einer Leinwand", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "Diashow…" }).click();
    await page.getByRole("button", { name: "Abspielen" }).click();
    await expect(page.getByRole("dialog", { name: "Diashow-Wiedergabe" }).locator("canvas")).toBeVisible();
  });

  test("Abbruch des Speichern-unter-Dialogs löst keinen export_slideshow_video-Aufruf aus", async ({ page }) => {
    await setUpWithSelectedPhoto(page, { pickSaveFilePathResult: null });
    await page.getByRole("button", { name: "Diashow…" }).click();
    await page.getByRole("button", { name: "Als Video exportieren (MP4)" }).click();

    const log = await getMockInvokeLog(page);
    expect(log.find((e) => e.cmd === "export_slideshow_video")).toBeUndefined();
  });

  test("Video-Export sendet die gewählten Einstellungen und zeigt das Ergebnis", async ({ page }) => {
    await setUpWithSelectedPhoto(page, {
      pickSaveFilePathResult: "/home/user/Videos/Diashow.mp4",
      pickFilePathResult: "/home/user/Musik/song.mp3",
    });
    await page.getByRole("button", { name: "Diashow…" }).click();

    await page.getByLabel("Dauer je Foto (s)").fill("5");
    await page.getByLabel("Übergang").selectOption("cut");
    await page.getByRole("checkbox", { name: "Ken-Burns-Effekt" }).uncheck();
    await page.getByLabel("Video-Auflösung").selectOption("1280x720");
    await page.getByLabel("Bildrate (fps)").selectOption("30");
    await page.getByRole("button", { name: /Musikdatei wählen/ }).click();

    await page.getByRole("button", { name: "Als Video exportieren (MP4)" }).click();
    await expect(page.getByText("Video gespeichert: /mock/slideshow/Diashow.mp4 (10.0s)")).toBeVisible();

    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "export_slideshow_video");
    expect(call).toBeDefined();
    const args = call?.args as {
      photoIds: string[];
      destPath: string;
      options: { slideSeconds: number; kenBurns: boolean; transition: string; width: number; height: number; fps: number; musicPath?: string };
    };
    expect(args.photoIds).toEqual([PHOTO.id]);
    expect(args.destPath).toBe("/home/user/Videos/Diashow.mp4");
    expect(args.options.slideSeconds).toBe(5);
    expect(args.options.kenBurns).toBe(false);
    expect(args.options.transition).toBe("cut");
    expect(args.options.width).toBe(1280);
    expect(args.options.height).toBe(720);
    expect(args.options.fps).toBe(30);
    expect(args.options.musicPath).toBe("/home/user/Musik/song.mp3");
  });

  test("Intro-Text erfordert eine Schriftdatei und schickt beides mit", async ({ page }) => {
    await setUpWithSelectedPhoto(page, {
      pickSaveFilePathResult: "/home/user/Videos/Diashow.mp4",
      pickFilePathResult: "/home/user/Schriften/Font.ttf",
    });
    await page.getByRole("button", { name: "Diashow…" }).click();

    await page.getByRole("checkbox", { name: "Intro-Bildschirm" }).check();
    await page.getByPlaceholder("Text").fill("Willkommen");
    await page.getByRole("button", { name: "Schriftdatei wählen…" }).click();

    await page.getByRole("button", { name: "Als Video exportieren (MP4)" }).click();

    const log = await getMockInvokeLog(page);
    const call = log.find((e) => e.cmd === "export_slideshow_video");
    const args = call?.args as { options: { intro?: { text: string; fontPath?: string } } };
    expect(args.options.intro?.text).toBe("Willkommen");
    expect(args.options.intro?.fontPath).toBe("/home/user/Schriften/Font.ttf");
  });

  test("fehlgeschlagener Video-Export zeigt die Fehlermeldung", async ({ page }) => {
    await setUpWithSelectedPhoto(page, {
      pickSaveFilePathResult: "/home/user/Videos/Diashow.mp4",
      slideshowVideoShouldFail: true,
    });
    await page.getByRole("button", { name: "Diashow…" }).click();
    await page.getByRole("button", { name: "Als Video exportieren (MP4)" }).click();
    await expect(page.getByText(/Fehler: Test-Stub: Video-Export fehlgeschlagen/)).toBeVisible();
  });
});
