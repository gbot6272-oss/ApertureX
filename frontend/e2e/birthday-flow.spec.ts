import { expect, test } from "@playwright/test";

import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const FOLDER_PATH = "/home/user/Fotos/Urlaub";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000101",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  missing: false,
  camera_model: "Canon EOS",
};

/** Die Deko ist im Mock standardmäßig aus (siehe `tauri-mock.ts`) —
 * diese Tests schalten sie gezielt ein. */
const WITH_DECORATIONS = {
  folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
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
  },
};

/**
 * Deckt die Geburtstagsausgabe ab (`DECISIONS.md` ADR-0071). Die
 * Konfetti-Physik selbst ist in `lib/confetti.test.ts` abgedeckt — hier
 * geht es um die Zusagen, die nur die laufende App halten kann, und die
 * wichtigste davon ist die dritte: **die Deko darf nichts blockieren.**
 */
test.describe("Geburtstagsausgabe (ADR-0071)", () => {
  test("gratuliert beim Öffnen mit großer Schrift und geblurrtem Hintergrund", async ({ page }) => {
    await installTauriMock(page, WITH_DECORATIONS);
    await page.goto("/");

    const welcome = page.getByTestId("birthday-welcome");
    await expect(welcome).toBeVisible();
    await expect(page.getByTestId("birthday-welcome-title")).toHaveText("Happy Birthday Papa");

    // Der geblurrte Hintergrund ist der Kern der Anforderung — geprüft
    // am tatsächlich berechneten Stil, nicht an der Klassenliste.
    const backdrop = await welcome.evaluate((el) => getComputedStyle(el).backdropFilter);
    expect(backdrop).toContain("blur");

    // Konfetti-Leinwand und Luftballons sind da.
    await expect(page.getByTestId("confetti-layer")).toBeAttached();
    await expect(page.getByTestId("balloon-layer")).toBeAttached();
  });

  test("lässt sich wegklicken und kommt nicht wieder", async ({ page }) => {
    await installTauriMock(page, WITH_DECORATIONS);
    await page.goto("/");

    await expect(page.getByTestId("birthday-welcome")).toBeVisible();
    await page.getByTestId("birthday-welcome-dismiss").click();
    await expect(page.getByTestId("birthday-welcome")).toHaveCount(0);

    // Ein Ordnerwechsel löst viele Store-Updates aus — die Begrüßung
    // darf dabei nicht erneut auftauchen.
    await page.getByRole("button", { name: /Urlaub/ }).click();
    await expect(page.getByTestId("birthday-welcome")).toHaveCount(0);
  });

  /**
   * Die wichtigste Zusage dieser Fassung. Die Begrüßung erscheint bei
   * JEDEM Start und liegt über allem — wenn sie Klicks abfinge, wäre
   * die App in den ersten fünf Sekunden unbenutzbar.
   *
   * **Der Test musste zweimal geschrieben werden.** Die erste Fassung
   * klickte einfach und prüfte das Ergebnis — und war wertlos:
   * Playwrights Klick wartet auf Klickbarkeit, also wartete er
   * stillschweigend die 5,2 Sekunden ab, bis sich die Begrüßung von
   * selbst schließt, und klickte dann. Mit abgeschaltetem
   * `pointer-events-none` lief er genauso grün durch. Die Gegenprobe
   * hat das gezeigt.
   *
   * Deshalb jetzt mit **kurzer Frist** (deutlich unter der Standzeit
   * der Begrüßung) und mit der Prüfung, dass die Begrüßung beim
   * Auslösen **noch sichtbar war**. Fängt die Fläche Ereignisse ab,
   * läuft der Klick in die Frist und der Test schlägt an.
   */
  test("blockiert die Bedienung nicht, während sie noch sichtbar ist", async ({ page }) => {
    await installTauriMock(page, WITH_DECORATIONS);
    await page.goto("/");
    const welcome = page.getByTestId("birthday-welcome");
    await expect(welcome).toBeVisible();

    // Kein `force: true` (das würde die Klickbarkeitsprüfung gerade
    // überspringen) und eine Frist weit unter den 5,2 s Standzeit.
    await page.getByRole("button", { name: /Urlaub/ }).click({ timeout: 1200 });
    await expect(page.getByRole("img", { name: PHOTO.filename })).toBeVisible({ timeout: 1200 });

    // Und die Begrüßung lag dabei wirklich noch darüber: sie wird
    // durch den Klick geschlossen (Zeiger-Lauscher am `window`), also
    // ist sie jetzt weg — das belegt, dass sie beim Klick noch da war
    // und der Klick trotzdem durchkam.
    await expect(welcome).toHaveCount(0);
  });

  /**
   * Dieselbe Zusage noch einmal, aber ohne jede Zeitabhängigkeit: die
   * Fläche nimmt keine Zeigerereignisse an, der Knopf darin schon.
   * Ein Test, der nur klickt, kann grün werden, weil er gewartet hat
   * (siehe den Kommentar beim Test darüber) — dieser kann es nicht.
   */
  test("die Begrüßungsfläche nimmt keine Zeigerereignisse an, ihr Knopf schon", async ({ page }) => {
    await installTauriMock(page, WITH_DECORATIONS);
    await page.goto("/");
    const welcome = page.getByTestId("birthday-welcome");
    await expect(welcome).toBeVisible();

    await expect(welcome).toHaveCSS("pointer-events", "none");
    await expect(page.getByTestId("birthday-welcome-dismiss")).toHaveCSS("pointer-events", "auto");

    // Und die Deko-Schichten ebenfalls nicht — sonst läge statt der
    // Begrüßung die Konfetti-Leinwand im Weg.
    await expect(page.getByTestId("confetti-layer")).toHaveCSS("pointer-events", "none");
    await expect(page.getByTestId("balloon-layer")).toHaveCSS("pointer-events", "none");
  });

  test("zeigt ohne Deko-Einstellung weder Begrüßung noch Konfetti noch Luftballons", async ({ page }) => {
    // Der Mock-Standard hat `birthday_decorations: false`.
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();

    await expect(page.getByTestId("birthday-welcome")).toHaveCount(0);
    await expect(page.getByTestId("confetti-layer")).toHaveCount(0);
    await expect(page.getByTestId("balloon-layer")).toHaveCount(0);
  });

  /** Reduzierte Bewegung schlägt die Deko — Zugänglichkeit geht vor
   * Schmuck, und zwar auch, wenn die Deko-Einstellung an ist. */
  test("schaltet bei reduzierter Bewegung alle Bewegung ab", async ({ page }) => {
    await installTauriMock(page, {
      ...WITH_DECORATIONS,
      uiSettings: { ...WITH_DECORATIONS.uiSettings, reduced_motion: true },
    });
    await page.goto("/");
    await page.getByRole("button", { name: /Urlaub/ }).click();

    await expect(page.getByTestId("confetti-layer")).toHaveCount(0);
    await expect(page.getByTestId("balloon-layer")).toHaveCount(0);
  });

  test("das Geburtstags-Theme ist die Vorgabe und bleibt umschaltbar", async ({ page }) => {
    await installTauriMock(page, WITH_DECORATIONS);
    await page.goto("/");

    await expect(page.locator("html")).toHaveAttribute("data-theme", "birthday");

    await page.getByTestId("birthday-welcome-dismiss").click();
    await page.getByRole("button", { name: "Einstellungen…" }).click();
    const dialog = page.getByRole("dialog", { name: "Einstellungen" });
    await dialog.getByLabel("Theme").selectOption("dark");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  });
});

/** Die Kamera-Vorgabe (ADR-0071 B4) — dass sie inhaltlich stimmt,
 * prüft `apx-app`s `birthday.rs` am echten Rendern; hier geht es nur
 * darum, dass sie in der Oberfläche ankommt, mit Abbildung. */
test.describe("Kamera-Standard der Geburtstagsausgabe", () => {
  test("zeigt „Canon EOS“ mit einer Kamera-Abbildung", async ({ page }) => {
    await installTauriMock(page, {
      folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
      photosByFolder: { [FOLDER_ID]: [PHOTO] },
    });
    await page.goto("/");

    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: /Standardentwicklung je Kamera/ }).click();
    const dialog = page.getByRole("dialog", { name: "Standardentwicklung je Kamera" });

    const list = dialog.getByTestId("camera-defaults-list");
    await expect(list).toContainText("Canon EOS");
    // Die Abbildung ist Teil der Anforderung, nicht Beiwerk.
    await expect(list.getByTestId("camera-illustration")).toHaveCount(1);
  });
});
