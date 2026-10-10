/**
 * **Die Leistungs-Diagnose muss benennen, was sie findet** (ADR-0073).
 *
 * Eine Messung, die Zahlen zeigt und den Nutzer mit ihnen allein lässt,
 * hat nichts abgenommen. Geprüft wird deshalb nicht nur, dass gemessen
 * wird, sondern dass der wichtigste Befund — ein Debug-Build — als
 * solcher benannt wird und nicht in einer Zahlenkolonne untergeht.
 */
import { test, expect } from "@playwright/test";
import { installTauriMock, openOverflowMenu } from "./tauri-mock";

const FOLDER_ID = "01977f4a-0000-7000-8000-000000000001";
const PHOTO = {
  id: "01977f4a-0000-7000-8000-000000000101",
  filename: "IMG_0001.CR3",
  width: 6000,
  height: 4000,
  missing: false,
  camera_model: "Canon EOS",
  content_hash: "hash-diagnose-1",
  captured_at: "2024-05-07T12:00:00Z",
};

async function openDialog(
  page: import("@playwright/test").Page,
  report?: Record<string, number | boolean>,
) {
  await installTauriMock(page, {
    folders: [{ id: FOLDER_ID, path: "/home/user/Fotos/Urlaub", photo_count: 1 }],
    photosByFolder: { [FOLDER_ID]: [PHOTO] },
    ...(report ? { performanceReport: report } : {}),
  });
  await page.goto("/");
  await openOverflowMenu(page);
  await page.getByRole("menuitem", { name: "Leistung prüfen…" }).click();
  await expect(page.getByRole("dialog", { name: "Leistung prüfen" })).toBeVisible();
}

test("meldet einen optimierten Build ohne Befund", async ({ page }) => {
  await openDialog(page);
  await page.getByTestId("performance-run").click();
  await expect(page.getByTestId("performance-report")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("performance-build")).toHaveText("Release (optimiert)");
  await expect(page.getByTestId("performance-debug-warning")).toHaveCount(0);
});

test("benennt einen Debug-Build als wahrscheinlichste Ursache", async ({ page }) => {
  // Der eine Befund, der eine Antwort ist statt einer Zahl: ein
  // Debug-Build erklärt für sich genommen einen Faktor, gegen den jede
  // andere Maßnahme verpufft. Er darf deshalb nicht als weitere Zeile
  // in der Liste erscheinen, sondern muss vorn stehen und sagen, was zu
  // tun ist.
  await openDialog(page, {
    debug_build: true,
    threads: 4,
    bench_megapixels: 12,
    develop_neutral_ms: 2400,
    develop_loaded_ms: 3100,
    megapixels_per_second: 5,
  });
  await page.getByTestId("performance-run").click();
  const warning = page.getByTestId("performance-debug-warning");
  await expect(warning).toBeVisible({ timeout: 20_000 });
  await expect(warning).toContainText("Debug-Build");
  // Ohne den konkreten Befehl wäre es eine Diagnose ohne Therapie.
  await expect(warning).toContainText("cargo tauri build");
  await expect(page.getByTestId("performance-build")).toHaveText("Debug (nicht optimiert)");
});

test("misst die Bildzeit der Oberfläche wirklich", async ({ page }) => {
  await openDialog(page);
  await page.getByTestId("performance-run").click();
  await expect(page.getByTestId("performance-report")).toBeVisible({ timeout: 20_000 });
  // Kein Vergleich gegen eine feste Zahl: die hinge an der Last der
  // Testmaschine. Geprüft wird, dass überhaupt ein Messwert entsteht
  // und nicht der Platzhalter stehen bleibt.
  const frame = await page.getByTestId("performance-frame").textContent();
  expect(frame).not.toBe("—");
  expect(frame).toMatch(/ms \(.*Bilder\/s\)/);
});
