import { expect, test } from "@playwright/test";

import { getMockInvokeLog, installTauriMock } from "./tauri-mock";

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

async function lastCommitFor(page: import("@playwright/test").Page, photoId: string) {
  const log = await getMockInvokeLog(page);
  return [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit" && (entry.args as { photoId: string }).photoId === photoId);
}

async function lastMasks(page: import("@playwright/test").Page): Promise<Array<{ name: string; visible: boolean }>> {
  const log = await getMockInvokeLog(page);
  const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
  const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
  return JSON.parse(edlJson).payload.masks;
}

/**
 * Deckt Phase 6 Schritt 3 ab (`DECISIONS.md` ADR-0032): Anlegen einer
 * Linearen-/Radialen-Verlauf-Maske, Ziehgriffe im Viewer (per
 * Tastatur-Feinsteuerung, analog zum bestehenden Freistellen-Werkzeug-
 * Test), Sichtbarkeit/Umbenennen/Löschen, sowie ein Regler für die
 * Maskeneigenen Grundeinstellungen.
 */
test.describe("Masken-Panel", () => {
  test("legt eine Lineare-Verlauf-Maske an, zeigt sie in der Liste und committet sofort", async ({ page }) => {
    await setUpWithSelectedPhoto(page);

    await expect(page.getByRole("complementary", { name: "Masken" })).toBeVisible();
    await expect(page.getByText("Keine Masken vorhanden.")).toBeVisible();

    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    await expect(page.getByRole("button", { name: "Linearer Verlauf", exact: true })).toBeVisible();
    const masks = await lastMasks(page);
    expect(masks).toHaveLength(1);
    expect(masks[0].name).toBe("Linearer Verlauf");
    expect(masks[0].visible).toBe(true);
  });

  test("Ziehgriff des Linearen Verlaufs verschiebt den Startpunkt per Pfeiltaste und committet", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    const startHandle = page.getByRole("slider", { name: "Linearer Verlauf: Startpunkt" });
    await expect(startHandle).toBeVisible();
    await startHandle.focus();
    await startHandle.press("ArrowRight");
    await startHandle.press("ArrowRight");

    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        return log.filter((entry) => entry.cmd === "apply_develop_edit").length;
      })
      .toBeGreaterThan(1);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const edlJson = (lastCommit?.args as { edlJson: string }).edlJson;
    const masks = JSON.parse(edlJson).payload.masks as Array<{ components: Array<{ geometry: { x1: number } }> }>;
    // Startwert war x1 = 0.5 (siehe `defaultLinearGradientGeometry`).
    expect(masks[0].components[0].geometry.x1).toBeGreaterThan(0.5);
  });

  test("Radialer-Verlauf-Ziehgriff vergrößert den Radius per Pfeiltaste und committet", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Radialer Verlauf" }).click();

    const radiusHandle = page.getByRole("slider", { name: "Radialer Verlauf: Radius" });
    await expect(radiusHandle).toBeVisible();
    await radiusHandle.focus();
    await radiusHandle.press("ArrowRight");

    await expect
      .poll(async () => {
        const masks = await lastMasks(page);
        return (masks[0] as unknown as { components: Array<{ geometry: { radius_x: number } }> }).components[0].geometry.radius_x;
      })
      .toBeGreaterThan(0.3);
  });

  test("Sichtbarkeit umschalten, umbenennen und löschen committen jeweils sofort", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    const visibilityButton = page.getByRole("button", { name: "Linearer Verlauf ausblenden" });
    await visibilityButton.click();
    await expect.poll(async () => (await lastMasks(page))[0].visible).toBe(false);

    page.once("dialog", (dialog) => {
      void dialog.accept("Mein Vordergrund");
    });
    await page.getByTitle("Umbenennen").click();
    await expect.poll(async () => (await lastMasks(page))[0].name).toBe("Mein Vordergrund");

    await page.getByRole("button", { name: "Mein Vordergrund löschen" }).click();
    await expect.poll(async () => (await lastMasks(page)).length).toBe(0);
    await expect(page.getByText("Keine Masken vorhanden.")).toBeVisible();
  });

  test("Maskeneigener Belichtung-Regler ändert nur die Anpassungen der ausgewählten Maske", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    const maskExposure = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" }).nth(1);
    await maskExposure.fill("0.8");
    await maskExposure.blur();

    await expect
      .poll(async () => {
        const masks = await lastMasks(page);
        return (masks[0] as unknown as { adjustments: { basic: { exposure_ev: number } } }).adjustments.basic.exposure_ev;
      })
      .toBeCloseTo(0.8, 2);

    // Der globale Belichtung-Regler bleibt bei 0 — nur die Maske hat sich geändert.
    const globalExposure = page.getByRole("spinbutton", { name: "Belichtung (Zahlenwert)" }).first();
    await expect(globalExposure).toHaveValue("0");
  });

  test("Pinselmaske: ein Ziehvorgang im Bild malt einen Strich, Entfernen committet erneut", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Pinsel" }).click();

    await expect(page.getByText("Ins Bild klicken und ziehen, um zu malen.")).toBeVisible();

    // Malen: ein kurzer Ziehvorgang neben der Bildmitte (dieselbe Annahme
    // wie beim Reparatur-Pinsel-Test: Bildmitte == Container-Mitte).
    const mainBox = await page.getByRole("main").boundingBox();
    if (!mainBox) throw new Error("Viewer-Container nicht gefunden");
    const centerX = mainBox.x + mainBox.width / 2;
    const centerY = mainBox.y + mainBox.height / 2;
    await page.mouse.move(centerX - 20, centerY - 20);
    await page.mouse.down();
    await page.mouse.move(centerX + 20, centerY + 10, { steps: 4 });
    await page.mouse.up();

    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        return log.filter((entry) => entry.cmd === "apply_develop_edit").length;
      })
      .toBeGreaterThan(1);

    const masks = await lastMasks(page);
    const strokes = (masks[0] as unknown as { components: Array<{ geometry: { strokes: Array<{ points: Array<{ x: number; y: number }> }> } }> }).components[0]
      .geometry.strokes;
    expect(strokes).toHaveLength(1);
    expect(strokes[0].points.length).toBeGreaterThan(0);

    await page.getByRole("button", { name: "Entfernen", exact: true }).click();

    await expect
      .poll(async () => {
        const masks = await lastMasks(page);
        return (masks[0] as unknown as { components: Array<{ geometry: { strokes: unknown[] } }> }).components[0].geometry.strokes.length;
      })
      .toBe(0);
  });

  test("Farbbereich-Maske: ein Bildklick nimmt die Zielfarbe auf, die Toleranz committet zusätzlich", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Farbbereich" }).click();

    const pickButton = page.getByRole("button", { name: "Farbe aufnehmen" });
    await expect(pickButton).toHaveAttribute("aria-pressed", "false");
    await pickButton.click();
    await expect(pickButton).toHaveAttribute("aria-pressed", "true");

    await page.getByRole("main").click();

    await expect(pickButton).toHaveAttribute("aria-pressed", "false");
    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        return log.filter((entry) => entry.cmd === "apply_develop_edit").length;
      })
      .toBeGreaterThan(0);

    type ColorRangeGeometry = { target_r: number; target_g: number; target_b: number; tolerance: number };
    let masks = await lastMasks(page);
    let geometry = (masks[0] as unknown as { components: Array<{ geometry: ColorRangeGeometry }> }).components[0].geometry;
    // Das Mock-Entwickeln-Bild ist warm-orange (180, 140, 100).
    expect(geometry.target_r).toBeCloseTo(180 / 255, 2);
    expect(geometry.target_g).toBeCloseTo(140 / 255, 2);
    expect(geometry.target_b).toBeCloseTo(100 / 255, 2);

    const toleranceInput = page.getByRole("spinbutton", { name: "Toleranz (%) (Zahlenwert)" });
    await toleranceInput.fill("40");
    await toleranceInput.blur();

    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        return log.filter((entry) => entry.cmd === "apply_develop_edit").length;
      })
      .toBeGreaterThan(1);
    masks = await lastMasks(page);
    geometry = (masks[0] as unknown as { components: Array<{ geometry: ColorRangeGeometry }> }).components[0].geometry;
    expect(geometry.tolerance).toBeCloseTo(0.4, 2);
  });

  test("Luminanzbereich-Maske: die Reglerwerte committen", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Luminanzbereich" }).click();

    const rangeMinInput = page.getByRole("spinbutton", { name: "Untere Grenze (%) (Zahlenwert)" });
    await rangeMinInput.fill("20");
    await rangeMinInput.blur();

    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        return log.filter((entry) => entry.cmd === "apply_develop_edit").length;
      })
      .toBeGreaterThan(1);

    const masks = await lastMasks(page);
    const geometry = (masks[0] as unknown as { components: Array<{ geometry: { range_min: number } }> }).components[0].geometry;
    expect(geometry.range_min).toBeCloseTo(0.2, 2);
  });

  test("Maskenkombination: eine zweite Komponente mit Subtrahieren+Invertieren committet, Mischmodus committet zusätzlich", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    await page.getByRole("button", { name: "+ Komponente: Farbbereich" }).click();

    type MaskWithComponents = {
      blend_mode: string;
      components: Array<{ geometry: { kind: string }; combine: string; invert: boolean }>;
    };
    let masks = (await lastMasks(page)) as unknown as MaskWithComponents[];
    expect(masks[0].components).toHaveLength(2);
    expect(masks[0].components[1].geometry.kind).toBe("ColorRange");

    await page.getByRole("combobox", { name: "Komponente 2: Verrechnung" }).selectOption("Subtract");
    await page.getByRole("checkbox", { name: "Komponente 2: Invertieren" }).check();

    await expect
      .poll(async () => {
        const log = await getMockInvokeLog(page);
        return log.filter((entry) => entry.cmd === "apply_develop_edit").length;
      })
      .toBeGreaterThan(2);
    masks = (await lastMasks(page)) as unknown as MaskWithComponents[];
    expect(masks[0].components[1].combine).toBe("Subtract");
    expect(masks[0].components[1].invert).toBe(true);

    await page.getByRole("combobox", { name: "Mischmodus" }).selectOption("Multiply");
    await expect
      .poll(async () => {
        const masks = (await lastMasks(page)) as unknown as MaskWithComponents[];
        return masks[0].blend_mode;
      })
      .toBe("Multiply");

    // Komponente wieder entfernen: nur noch eine Komponente übrig.
    await page.getByRole("button", { name: "Komponente 2 entfernen" }).click();
    await expect
      .poll(async () => {
        const masks = (await lastMasks(page)) as unknown as MaskWithComponents[];
        return masks[0].components.length;
      })
      .toBe(1);
  });

  test("Sechs-Sektionen-Regler: HSL-Band, Details-Schärfung und Color-Grading-Balance wirken nur auf die ausgewählte Maske", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    type MaskAdjustments = { hsl: { red: { hue: number } }; details: { sharpen_amount: number }; color_grading: { balance: number } };

    const hueInput = page.getByRole("spinbutton", { name: "Farbton (Zahlenwert)" }).nth(1);
    await hueInput.fill("30");
    await hueInput.blur();
    await expect
      .poll(async () => {
        const masks = (await lastMasks(page)) as unknown as Array<{ adjustments: MaskAdjustments }>;
        return masks[0].adjustments.hsl.red.hue;
      })
      .toBeCloseTo(30, 1);

    const sharpenInput = page.getByRole("spinbutton", { name: "Schärfung: Betrag (Zahlenwert)" }).nth(1);
    await sharpenInput.fill("40");
    await sharpenInput.blur();
    await expect
      .poll(async () => {
        const masks = (await lastMasks(page)) as unknown as Array<{ adjustments: MaskAdjustments }>;
        return masks[0].adjustments.details.sharpen_amount;
      })
      .toBeCloseTo(40, 1);

    const balanceInput = page.getByRole("spinbutton", { name: "Balance (Zahlenwert)" }).nth(1);
    await balanceInput.fill("15");
    await balanceInput.blur();
    await expect
      .poll(async () => {
        const masks = (await lastMasks(page)) as unknown as Array<{ adjustments: MaskAdjustments }>;
        return masks[0].adjustments.color_grading.balance;
      })
      .toBeCloseTo(15, 1);

    // Die globalen Regler bleiben unverändert — nur die Maske hat sich geändert.
    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const globalPayload = JSON.parse((lastCommit?.args as { edlJson: string }).edlJson).payload;
    expect(globalPayload.hsl.red.hue).toBe(0);
    expect(globalPayload.details.sharpen_amount).toBe(0);
    expect(globalPayload.color_grading.balance).toBe(0);
  });

  test("Farbmischer: ein Bildklick legt eine Region an der Maske an, nicht an den globalen Einstellungen", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    const addRegionButton = page.getByRole("button", { name: "Region hinzufügen" }).nth(1);
    await addRegionButton.click();
    await page.getByRole("main").click();

    type MaskWithColorMixer = { adjustments: { color_mixer: { regions: unknown[] } } };
    await expect
      .poll(async () => {
        const masks = (await lastMasks(page)) as unknown as MaskWithColorMixer[];
        return masks[0].adjustments.color_mixer.regions.length;
      })
      .toBe(1);

    const log = await getMockInvokeLog(page);
    const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
    const payload = JSON.parse((lastCommit?.args as { edlJson: string }).edlJson).payload;
    expect(payload.color_mixer.regions).toHaveLength(0);
  });

  test("Maskengruppen: Anlegen, Zuordnen, Ausblenden und Entfernen committen jeweils", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    page.once("dialog", (dialog) => void dialog.accept("Vordergrund"));
    await page.getByRole("button", { name: "+ Neue Gruppe" }).click();

    type MaskWithGroup = { group_id: string | null };
    type EdlWithGroups = { masks: MaskWithGroup[]; mask_groups: Array<{ id: string; name: string; visible: boolean }> };

    async function lastPayload(): Promise<EdlWithGroups> {
      const log = await getMockInvokeLog(page);
      const lastCommit = [...log].reverse().find((entry) => entry.cmd === "apply_develop_edit");
      return JSON.parse((lastCommit?.args as { edlJson: string }).edlJson).payload;
    }

    await expect.poll(async () => (await lastPayload()).mask_groups).toHaveLength(1);
    const groupId = (await lastPayload()).mask_groups[0]!.id;

    await page.getByRole("combobox", { name: /: Gruppe$/ }).selectOption(groupId);
    await expect.poll(async () => (await lastPayload()).masks[0]!.group_id).toBe(groupId);

    const groupVisibilityButton = page.getByRole("button", { name: "Gruppe Vordergrund ausblenden" });
    await groupVisibilityButton.click();
    await expect.poll(async () => (await lastPayload()).mask_groups[0]!.visible).toBe(false);

    await page.getByRole("button", { name: "Gruppe Vordergrund entfernen" }).click();
    await expect.poll(async () => (await lastPayload()).mask_groups).toHaveLength(0);
    await expect.poll(async () => (await lastPayload()).masks[0]!.group_id).toBeNull();
  });

  test("Maske duplizieren legt eine Kopie mit eigener ID an und committet sofort", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    await page.getByTitle("Duplizieren").click();

    const masks = (await lastMasks(page)) as unknown as Array<{ id: string; name: string }>;
    expect(masks).toHaveLength(2);
    expect(masks[1]!.name).toContain("(Kopie)");
    expect(masks[1]!.id).not.toBe(masks[0]!.id);
  });

  test("Baustein speichern und anwenden legt eine neue Maske mit derselben Geometrie an", async ({ page }) => {
    await setUpWithSelectedPhoto(page);
    await page.getByRole("button", { name: "+ Radialer Verlauf" }).click();

    page.once("dialog", (dialog) => void dialog.accept("Mein Vignette-Baustein"));
    await page.getByRole("button", { name: "Aktuelle Maske als Baustein speichern" }).click();

    await page.getByRole("button", { name: "Mein Vignette-Baustein", exact: true }).click();

    type MaskWithComponents = { components: Array<{ geometry: { kind: string } }> };
    await expect
      .poll(async () => {
        const masks = (await lastMasks(page)) as unknown as MaskWithComponents[];
        return masks.length;
      })
      .toBe(2);
    const masks = (await lastMasks(page)) as unknown as MaskWithComponents[];
    expect(masks[1]!.components[0]!.geometry.kind).toBe("RadialGradient");
  });

  test("Auf anderes Foto übertragen kopiert die Maske ins EDL des Zielfotos", async ({ page }) => {
    await setUpWithSelectedPhoto(page, [PHOTO_2]);
    await page.getByRole("button", { name: "+ Linearer Verlauf" }).click();

    await page.getByRole("combobox", { name: "Zielfoto für Maskenübertragung" }).selectOption(PHOTO_2.id);
    await page.getByRole("button", { name: "Übertragen" }).click();

    type MaskWithComponents = { components: Array<{ geometry: { kind: string } }> };
    await expect
      .poll(async () => {
        const commit = await lastCommitFor(page, PHOTO_2.id);
        return commit !== undefined;
      })
      .toBe(true);

    const commit = await lastCommitFor(page, PHOTO_2.id);
    const targetPayload = JSON.parse((commit?.args as { edlJson: string }).edlJson).payload;
    const targetMasks = targetPayload.masks as MaskWithComponents[];
    expect(targetMasks).toHaveLength(1);
    expect(targetMasks[0]!.components[0]!.geometry.kind).toBe("LinearGradient");

    // Das Ursprungsfoto behält seine eigene, unveränderte Maskenliste.
    const originalMasks = await lastMasks(page);
    expect(originalMasks).toHaveLength(1);
  });
});
