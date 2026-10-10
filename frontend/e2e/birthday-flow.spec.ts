import { expect, test } from "@playwright/test";

import { emitMockEvent, installTauriMock, openOverflowMenu } from "./tauri-mock";

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
    // Fest auf "full" (ADR-0073): "auto" startet eine Bildzeit-Messung,
    // deren Ergebnis von der Last der Testmaschine abhaengt — die Deko
    // waere dann je nach Tagesform da oder nicht.
    effect_level: "full" as const,
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

/**
 * Das Tutorial (ADR-0071 B6). Die alte Tour hatte sieben Schritte und
 * hörte dort auf, wo die Arbeit anfängt — jetzt sind es fünfzehn, dem
 * Arbeitsweg nach geordnet. Geprüft wird, dass sie wirklich
 * durchläuft: ein Schritt mit fehlendem Übersetzungsschlüssel oder
 * einem Ziel, das abstürzt, würde hier auffallen.
 */
test.describe("Einführungstour der Geburtstagsausgabe", () => {
  const FIRST_RUN = {
    folders: [{ id: FOLDER_ID, path: FOLDER_PATH, photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
    uiSettings: { ...WITH_DECORATIONS.uiSettings, onboarding_seen: false, birthday_decorations: false },
  };

  test("startet beim ersten Start von selbst und hat fünfzehn Schritte", async ({ page }) => {
    await installTauriMock(page, FIRST_RUN);
    await page.goto("/");

    await expect(page.getByText("Schritt 1 von 15")).toBeVisible();
    await expect(page.getByText("Alles Gute, Papa!")).toBeVisible();
  });

  test("läuft von Anfang bis Ende durch, ohne an einem Schritt hängenzubleiben", async ({ page }) => {
    await installTauriMock(page, FIRST_RUN);
    await page.goto("/");

    for (let step = 1; step <= 15; step += 1) {
      await expect(page.getByText(`Schritt ${step} von 15`)).toBeVisible();
      // Jeder Schritt muss einen echten Text haben — ein fehlender
      // Übersetzungsschlüssel würde den Schlüssel selbst anzeigen.
      const body = page.getByTestId("onboarding-tour-body");
      await expect(body).not.toContainText("onboarding.tour.");
      if (step < 15) {
        await page.getByRole("button", { name: "Weiter", exact: true }).click();
      }
    }
    await page.getByRole("button", { name: "Fertig", exact: true }).click();
    await expect(page.getByText(/Schritt \d+ von 15/)).toHaveCount(0);
  });

  /**
   * **Dieser Test deckt einen Altfehler ab, den der Tour-Ausbau
   * aufgedeckt hat** (siehe `index.css`s Kommentar zu
   * `.apx-spotlight`): `.apx-glass-strong` setzt unlayered
   * `position: relative`, und Tailwinds `fixed`-Utility ist in
   * Tailwind v4 nachrangig. Die vier Abdunkel-Rechtecke des
   * Scheinwerfers und die Karte lagen deshalb seit Phase 21 im
   * Textfluss statt über dem Bildschirm — der Scheinwerfer hat nie
   * einen Ausschnitt gezeigt. Aufgefallen ist es nie, weil kein Test
   * die Geometrie gemessen hat, sondern nur Textsichtbarkeit.
   *
   * Also wird hier gemessen: `position` muss `fixed` sein, und die
   * Karte muss in JEDEM Schritt vollständig im Fenster liegen.
   */
  test("die Karte liegt in jedem Schritt vollständig im Fenster", async ({ page }) => {
    await installTauriMock(page, FIRST_RUN);
    await page.goto("/");

    const card = page.getByRole("dialog").filter({ hasText: "Schritt" }).first();
    const viewport = await page.evaluate(() => ({ w: window.innerWidth, h: window.innerHeight }));

    for (let step = 1; step <= 15; step += 1) {
      await expect(page.getByText(`Schritt ${step} von 15`)).toBeVisible();
      // Die Karte wird per GSAP an ihren Platz bewegt — erst messen,
      // wenn sie dort ist.
      await expect(card).toHaveCSS("position", "fixed");
      await page.waitForTimeout(650);
      const box = await card.boundingBox();
      expect(box, `Schritt ${step}: keine Kartenfläche`).not.toBeNull();
      expect(box!.y, `Schritt ${step}: Karte oben außerhalb`).toBeGreaterThanOrEqual(0);
      expect(box!.x, `Schritt ${step}: Karte links außerhalb`).toBeGreaterThanOrEqual(0);
      expect(box!.y + box!.height, `Schritt ${step}: Karte unten außerhalb`).toBeLessThanOrEqual(viewport.h);
      expect(box!.x + box!.width, `Schritt ${step}: Karte rechts außerhalb`).toBeLessThanOrEqual(viewport.w);
      if (step < 15) await page.getByRole("button", { name: "Weiter", exact: true }).click();
    }
  });

  test("der Schritt zu den drei Looks nennt sie beim Namen", async ({ page }) => {
    await installTauriMock(page, FIRST_RUN);
    await page.goto("/");

    // Zum Presets-Schritt (8 von 15) vorklicken.
    for (let step = 1; step < 8; step += 1) {
      await page.getByRole("button", { name: "Weiter", exact: true }).click();
    }
    const body = page.getByTestId("onboarding-tour-body");
    await expect(body).toContainText("Goldene Stunde");
    await expect(body).toContainText("Familienfoto");
    await expect(body).toContainText("Erinnerung");
  });
});

/**
 * Abschleifen (ADR-0071 B7). Die Deko läuft in einer App, die
 * gleichzeitig RAW-Dateien dekodiert — sie darf nichts blockieren und
 * nichts liegen lassen. Diese Tests messen das, statt es zu behaupten.
 */
test.describe("Geburtstags-Deko: Tempo und Sauberkeit", () => {
  test("ein Konfetti-Ausbruch blockiert die Bedienung nicht", async ({ page }) => {
    await installTauriMock(page, WITH_DECORATIONS);
    await page.goto("/");
    await page.getByTestId("birthday-welcome-dismiss").click();
    await page.getByRole("button", { name: /Urlaub/ }).click();

    // Ein fertiger Import löst das Fest aus (`store.finishImport`).
    await emitMockEvent(page, "import:finished", {
      imported: 12,
      skipped: 0,
      error_count: 0,
      cancelled: false,
      duplicate_count: 0,
    });
    await expect(page.getByTestId("confetti-layer")).toBeAttached();

    /**
     * **Hier ist ein zweiter Altfehler aufgefallen** (siehe
     * `Header.tsx`s Kommentar dazu): die Import-Ergebniszeile war
     * `shrink-0` ohne Breitenbegrenzung und schob das Überlauf-Menü
     * und „Einstellungen" bei 1280 Pixel rechts aus dem Fenster.
     * Beide waren nach jedem Import unerreichbar. Deshalb wird die
     * Erreichbarkeit hier zuerst gemessen, bevor geklickt wird.
     */
    const viewport = await page.evaluate(() => window.innerWidth);
    for (const name of ["Weitere Funktionen", "Einstellungen…"]) {
      const box = await page.getByRole("button", { name }).boundingBox();
      expect(box, `${name}: keine Fläche`).not.toBeNull();
      expect(box!.x + box!.width, `${name} liegt nach dem Import außerhalb des Fensters`).toBeLessThanOrEqual(
        viewport,
      );
    }

    // Mitten im fliegenden Konfetti: ein Dialog muss sich in unter
    // einer Sekunde öffnen lassen. Kein `force` — Playwright prüft
    // echte Klickbarkeit, die Leinwand liegt über allem.
    await openOverflowMenu(page);
    await page.getByRole("menuitem", { name: /Katalog-Gesundheit/ }).click({ timeout: 1000 });
    await expect(page.getByRole("dialog", { name: "Katalog-Gesundheit" })).toBeVisible({ timeout: 1000 });
  });

  /** Abschalten muss wirklich abschalten: kein Element, keine
   * Leinwand, keine laufende Schleife. Und wiederholtes Umschalten
   * darf keine zweite Leinwand hinterlassen. */
  test("Deko abschalten räumt die Leinwand ab, mehrfaches Umschalten hinterlässt nur eine", async ({ page }) => {
    await installTauriMock(page, WITH_DECORATIONS);
    await page.goto("/");
    await page.getByTestId("birthday-welcome-dismiss").click();

    const toggle = () => page.getByRole("checkbox", { name: "Geburtstags-Deko" });
    await page.getByRole("button", { name: "Einstellungen…" }).click();

    for (let round = 0; round < 3; round += 1) {
      await toggle().uncheck();
      await expect(page.getByTestId("confetti-layer")).toHaveCount(0);
      await expect(page.getByTestId("balloon-layer")).toHaveCount(0);
      await toggle().check();
      await expect(page.getByTestId("confetti-layer")).toHaveCount(1);
      await expect(page.getByTestId("balloon-layer")).toHaveCount(1);
    }
  });

  /**
   * Der Start darf durch die Deko nicht langsamer werden. Gemessen
   * wird die Zeit bis zum ersten bedienbaren Zustand (der Ordner in
   * der Seitenleiste ist klickbar), einmal mit und einmal ohne Deko.
   *
   * Die Schranke ist bewusst großzügig (300 ms Unterschied): in CI
   * schwanken solche Messungen, und ein knapper Grenzwert wäre ein
   * flackernder Test. Was er fangen soll, ist eine Größenordnung —
   * eine Deko, die den Start um Sekunden verzögert, nicht zwanzig
   * Millisekunden Rauschen.
   */
  test("die Deko verzögert den Start nicht messbar", async ({ page }) => {
    async function timeToInteractive(decorations: boolean): Promise<number> {
      await installTauriMock(page, {
        ...WITH_DECORATIONS,
        uiSettings: { ...WITH_DECORATIONS.uiSettings, birthday_decorations: decorations },
      });
      const started = Date.now();
      await page.goto("/");
      await page.getByRole("button", { name: /Urlaub/ }).waitFor({ state: "visible" });
      return Date.now() - started;
    }

    const without = await timeToInteractive(false);
    const with_ = await timeToInteractive(true);
    // Für das Protokoll im Testlauf — eine Zahl, die man lesen kann,
    // ist mehr wert als ein grünes Häkchen allein.
    console.log(`Start bis bedienbar: ohne Deko ${without} ms, mit Deko ${with_} ms`);
    expect(with_).toBeLessThan(without + 300);
  });
});

/**
 * **Die Ballons waren nie zu sehen** (ADR-0078).
 *
 * Nutzer-Rückmeldung: „ehrlich gesagt sehe ich die Luftballons gar
 * nicht — habe ich auch nie."
 *
 * **Zwei Prüfungen, die nicht taugen, und warum.**
 *
 * 1. `document.elementFromPoint` auf der Mitte eines Ballons liefert
 *    `MAIN`, `FOOTER` oder einen `BUTTON`. Das sieht nach einem Beweis
 *    aus und ist keiner: die Schicht trägt `pointer-events: none`, der
 *    Strahl geht also IMMER hindurch — unabhängig davon, was man
 *    sieht.
 * 2. „Zwei Aufnahmen unterscheiden sich" ist zu schwach. Bei
 *    `z-index: 0` unterschieden sie sich auch, nämlich in 2.462 von
 *    1.440.000 Bildpunkten — 0,17 % des Fensters, bei 22 % Deckkraft.
 *    Also genau das, was der Nutzer als „sehe ich nicht" beschreibt.
 *
 * Gezählt werden deshalb die Bildpunkte, die sich ändern, wenn man
 * ALLEIN die Ballonschicht unsichtbar schaltet — derselbe Zustand,
 * dieselbe Begrüßung, dieselbe Animationsphase, eine Variable.
 *
 * Gemessen bei 1600×900: `z-index: 0` ergab 2.462 Bildpunkte,
 * `z-index: 35` ergibt 8.909. Sechs Ballons hätten als volle Fläche
 * rund 12.000 — vorher waren also gut vier Fünftel jedes Ballons
 * verdeckt.
 */
test("Luftballons sind nicht nur vorhanden, sondern wirklich zu sehen", async ({ page }) => {
  test.setTimeout(120_000);
  await page.setViewportSize({ width: 1600, height: 900 });
  await installTauriMock(page, WITH_DECORATIONS);
  await page.goto("/");
  await expect(page.getByTestId("balloon-layer")).toBeAttached();
  // Die Begrüßung blendet sich nach gut fünf Sekunden aus; vorher
  // deckt ihr geblurrter Hintergrund das halbe Bild ab. Danach stehen
  // auch die Ballons nicht mehr alle unter dem unteren Rand.
  await page.waitForTimeout(6000);
  await page.evaluate(() => {
    document.querySelectorAll<HTMLElement>(".apx-balloon").forEach((balloon) => {
      balloon.style.animationPlayState = "paused";
    });
  });

  async function aufnahme(sichtbar: boolean): Promise<string> {
    await page.evaluate((zeigen) => {
      const layer = document.querySelector<HTMLElement>(".apx-balloon-layer");
      if (layer) layer.style.visibility = zeigen ? "visible" : "hidden";
    }, sichtbar);
    return (await page.screenshot()).toString("base64");
  }

  const mit = await aufnahme(true);
  const ohne = await aufnahme(false);

  const sichtbarePixel = await page.evaluate(
    async ([a, b]) => {
      const laden = async (daten: string) => {
        const bild = new Image();
        bild.src = `data:image/png;base64,${daten}`;
        await bild.decode();
        const leinwand = document.createElement("canvas");
        leinwand.width = bild.width;
        leinwand.height = bild.height;
        const ctx = leinwand.getContext("2d");
        if (!ctx) throw new Error("kein 2D-Kontext");
        ctx.drawImage(bild, 0, 0);
        return ctx.getImageData(0, 0, leinwand.width, leinwand.height).data;
      };
      const x = await laden(a ?? "");
      const y = await laden(b ?? "");
      let anders = 0;
      for (let i = 0; i < x.length; i += 4) {
        const delta =
          Math.abs(x[i]! - y[i]!) + Math.abs(x[i + 1]! - y[i + 1]!) + Math.abs(x[i + 2]! - y[i + 2]!);
        // Schwelle 2 statt 0: JPEG-freie PNGs sind zwar exakt, aber
        // Unterpixel-Glättung an den Kanten soll nicht mitzählen.
        if (delta > 2) anders += 1;
      }
      return anders;
    },
    [mit, ohne],
  );

  // 4.000 liegt sauber zwischen den beiden gemessenen Werten (2.462
  // verdeckt, 8.909 frei) und ist damit eine Schwelle, die den
  // Unterschied wirklich trennt, statt ihn nur zu streifen.
  expect(
    sichtbarePixel,
    "die Luftballons sind größtenteils von deckenden Flächen verdeckt",
  ).toBeGreaterThan(4000);
});
