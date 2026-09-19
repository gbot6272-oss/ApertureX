/**
 * Wasserzeichen-Vorlagen (Phase 33 F5) — Typ, Vorgabewerte und die
 * Geometrie für die Live-Vorschau.
 *
 * **Warum die Geometrie hier noch einmal steht.** Gerechnet wird beim
 * Export in Rust (`apx_export::watermark_layout`). Die Vorschau im
 * Dialog kann das nicht aufrufen — sie zeichnet auf ein Canvas, während
 * man an den Reglern zieht, und ein IPC-Aufruf pro Regleranschlag wäre
 * weder schnell noch nötig. Die Regeln sind so klein, dass sie sich
 * beschreiben lassen (Prozent der kürzeren Kante, zentriertes
 * Kachelmuster), und sie sind hier wie dort getestet. Das ist die
 * bewusste Abwägung: zwei kleine, je getestete Implementierungen einer
 * Formel gegen eine Vorschau, die bei jedem Pixel ruckelt.
 *
 * Gespeichert wird eine Vorlage als gewöhnlicher `templates`-Eintrag der
 * Art `"watermark"` — wie die Metadaten-Vorgaben aus F4, aus demselben
 * Grund (siehe `crates/apx-app/src/metadata_preset.rs`).
 */

import type { WatermarkPosition } from "./tauri";

export interface WatermarkTemplate {
  /** `"text"` oder `"image"`. */
  mode: "text" | "image";
  text: string;
  fontPath: string;
  imagePath: string;
  /** `[R, G, B]`, 0..255. */
  color: [number, number, number];
  position: WatermarkPosition;
  opacity: number;
  /** Prozent der kürzeren Bildkante. */
  sizePercent: number;
  marginPercent: number;
  tile: boolean;
  tileSpacingPercent: number;
  rotationDegrees: number;
}

export const DEFAULT_WATERMARK_TEMPLATE: WatermarkTemplate = {
  mode: "text",
  text: "© Aperture X",
  fontPath: "",
  imagePath: "",
  color: [255, 255, 255],
  position: "bottom_right",
  opacity: 0.7,
  sizePercent: 5,
  marginPercent: 3,
  tile: false,
  tileSpacingPercent: 5,
  rotationDegrees: -30,
};

/** Ein Rechteck in Vorschaukoordinaten. */
export interface PlacedRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Die Kantenlänge in Pixeln, die `percent` der kürzeren Kante entspricht.
 * Spiegelt `margin_px`/`font_size_px` aus dem Rust-Modul.
 */
export function percentOfShortEdge(canvasW: number, canvasH: number, percent: number): number {
  return (Math.min(canvasW, canvasH) * percent) / 100;
}

/**
 * Die Position eines Overlays an einer der fünf festen Stellen.
 * Spiegelt `watermark::origin_for`.
 */
export function placeAtPosition(
  canvasW: number,
  canvasH: number,
  overlayW: number,
  overlayH: number,
  position: WatermarkPosition,
  margin: number,
): PlacedRect {
  const base = { width: overlayW, height: overlayH };
  switch (position) {
    case "top_left":
      return { ...base, x: margin, y: margin };
    case "top_right":
      return { ...base, x: canvasW - overlayW - margin, y: margin };
    case "bottom_left":
      return { ...base, x: margin, y: canvasH - overlayH - margin };
    case "bottom_right":
      return { ...base, x: canvasW - overlayW - margin, y: canvasH - overlayH - margin };
    case "center":
      return { ...base, x: (canvasW - overlayW) / 2, y: (canvasH - overlayH) / 2 };
  }
}

/**
 * Die Ursprünge aller Kacheln. Spiegelt `watermark_layout::tile_origins`
 * — inklusive der Zentrierung des Musters, deren Fehlen man in der
 * Vorschau sofort sähe (ganze Kachel oben links, angeschnittene unten
 * rechts).
 */
export function tileOrigins(
  canvasW: number,
  canvasH: number,
  tileW: number,
  tileH: number,
  spacing: number,
): { x: number; y: number }[] {
  if (tileW <= 0 || tileH <= 0) return [];
  const stepX = tileW + spacing;
  const stepY = tileH + spacing;
  const cols = Math.floor(canvasW / stepX) + 2;
  const rows = Math.floor(canvasH / stepY) + 2;
  const patternW = cols * stepX - spacing;
  const patternH = rows * stepY - spacing;
  const startX = (canvasW - patternW) / 2;
  const startY = (canvasH - patternH) / 2;

  const origins: { x: number; y: number }[] = [];
  for (let row = 0; row < rows; row += 1) {
    for (let col = 0; col < cols; col += 1) {
      origins.push({ x: startX + col * stepX, y: startY + row * stepY });
    }
  }
  return origins;
}

/**
 * Übersetzt eine Vorlage in die Exportoptionen-Felder.
 *
 * Eine eigene Funktion, damit der Export-Dialog eine Vorlage übernehmen
 * kann, ohne die Feldnamen der IPC-Grenze zu kennen — und damit ein Test
 * festhalten kann, dass eine Textvorlage ohne Schriftdatei gar nichts
 * setzt statt einen halben Auftrag zu schicken, den Rust dann ablehnt.
 */
export function templateToExportOptions(template: WatermarkTemplate): Record<string, unknown> {
  const common = {
    watermarkPosition: template.position,
    watermarkOpacity: template.opacity,
    watermarkSizePercent: template.sizePercent,
    watermarkMarginPercent: template.marginPercent,
    watermarkTile: template.tile,
    watermarkTileSpacingPercent: template.tileSpacingPercent,
    watermarkRotationDegrees: template.rotationDegrees,
  };
  if (template.mode === "text") {
    if (!template.text.trim() || !template.fontPath) return {};
    return {
      ...common,
      watermarkText: template.text,
      watermarkFontPath: template.fontPath,
      watermarkColor: template.color,
    };
  }
  if (!template.imagePath) return {};
  return { ...common, watermarkImagePath: template.imagePath };
}
