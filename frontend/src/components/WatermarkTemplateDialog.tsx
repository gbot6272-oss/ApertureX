import { AlertTriangle, Save, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { pickFilePath } from "../lib/tauri";
import type { TemplateDto, WatermarkPosition } from "../lib/tauri";
import {
  DEFAULT_WATERMARK_TEMPLATE,
  percentOfShortEdge,
  placeAtPosition,
  tileOrigins,
  type WatermarkTemplate,
} from "../lib/watermarkTemplate";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Wasserzeichen-Vorlagen (Phase 33 F5).
 *
 * **Warum es diesen Dialog überhaupt gibt.** Wasserzeichen konnte der
 * Export seit Phase 8 — aber mit Schriftgröße und Rand in absoluten
 * Pixeln. Dieselben Zahlen ergeben auf einem 6000-px-Druckexport einen
 * unlesbaren Fliegenschiss und auf einem 800-px-Web-Export einen Balken
 * quer durchs Bild. Genau deshalb ließ sich ein Wasserzeichen bisher
 * nicht als benannte Vorlage speichern: die Zahlen darin galten immer
 * nur für eine Ausgabegröße. Hier ist alles in Prozent der kürzeren
 * Kante — und damit über Exportgrößen hinweg dasselbe Bild.
 *
 * **Die Vorschau rechnet im Frontend.** Ein IPC-Aufruf pro
 * Regleranschlag wäre weder schnell noch nötig; die Formeln stehen in
 * `lib/watermarkTemplate.ts` und sind dort wie in Rust getestet. Die
 * Vorschau zeichnet einen Textkasten, keine echte Schriftrasterung — sie
 * zeigt Lage, Größe und Kachelung, nicht die Glyphen.
 */

const POSITION_LABELS: Record<WatermarkPosition, string> = {
  top_left: "Oben links",
  top_right: "Oben rechts",
  bottom_left: "Unten links",
  bottom_right: "Unten rechts",
  center: "Mitte",
};

const PREVIEW_W = 320;
const PREVIEW_H = 213;

function drawPreview(canvas: HTMLCanvasElement, template: WatermarkTemplate) {
  const dpr = Math.max(1, window.devicePixelRatio || 1);
  canvas.width = PREVIEW_W * dpr;
  canvas.height = PREVIEW_H * dpr;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, PREVIEW_W, PREVIEW_H);

  // Ein grobes Motiv, damit Deckkraft und Kontrast beurteilbar sind —
  // auf einer einfarbigen Fläche sieht jedes Wasserzeichen gut aus.
  const gradient = ctx.createLinearGradient(0, 0, PREVIEW_W, PREVIEW_H);
  gradient.addColorStop(0, "#2b3a4a");
  gradient.addColorStop(0.5, "#8fa3b8");
  gradient.addColorStop(1, "#1d2530");
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, PREVIEW_W, PREVIEW_H);

  const label = template.mode === "text" ? template.text || "Wasserzeichen" : "Logo";
  const fontSize = percentOfShortEdge(PREVIEW_W, PREVIEW_H, template.sizePercent);
  ctx.font = `${Math.max(6, fontSize)}px sans-serif`;
  const metrics = ctx.measureText(label);
  const overlayW = template.mode === "text" ? metrics.width : fontSize * 3;
  const overlayH = template.mode === "text" ? fontSize * 1.2 : fontSize;

  const [r, g, b] = template.color;
  ctx.globalAlpha = template.opacity;
  ctx.fillStyle = `rgb(${r}, ${g}, ${b})`;

  const paint = (x: number, y: number) => {
    if (template.mode === "text") {
      ctx.fillText(label, x, y + overlayH * 0.85);
    } else {
      ctx.fillRect(x, y, overlayW, overlayH);
    }
  };

  if (template.tile) {
    const spacing = percentOfShortEdge(PREVIEW_W, PREVIEW_H, template.tileSpacingPercent);
    ctx.save();
    // Um die Bildmitte drehen und das Muster im gedrehten System
    // aufbauen — dasselbe Ergebnis wie einzeln gedrehte Kacheln, aber
    // ohne für jede Kachel neu zu transformieren.
    ctx.translate(PREVIEW_W / 2, PREVIEW_H / 2);
    ctx.rotate((template.rotationDegrees * Math.PI) / 180);
    ctx.translate(-PREVIEW_W / 2, -PREVIEW_H / 2);
    // Das gedrehte Raster muss über das Bild hinausreichen, sonst
    // bleiben die Ecken leer.
    const overscan = Math.max(PREVIEW_W, PREVIEW_H);
    for (const origin of tileOrigins(
      PREVIEW_W + overscan,
      PREVIEW_H + overscan,
      overlayW,
      overlayH,
      spacing,
    )) {
      paint(origin.x - overscan / 2, origin.y - overscan / 2);
    }
    ctx.restore();
  } else {
    const margin = percentOfShortEdge(PREVIEW_W, PREVIEW_H, template.marginPercent);
    const rect = placeAtPosition(PREVIEW_W, PREVIEW_H, overlayW, overlayH, template.position, margin);
    paint(rect.x, rect.y);
  }
  ctx.globalAlpha = 1;
}

interface WatermarkTemplateDialogProps {
  open: boolean;
  onClose: () => void;
}

export function WatermarkTemplateDialog({ open, onClose }: WatermarkTemplateDialogProps) {
  const templatesByKind = useAppStore((s) => s.templatesByKind);
  const refreshTemplates = useAppStore((s) => s.refreshTemplates);
  const saveTemplateAction = useAppStore((s) => s.saveTemplateAction);
  const deleteTemplateAction = useAppStore((s) => s.deleteTemplateAction);

  const [template, setTemplate] = useState<WatermarkTemplate>(DEFAULT_WATERMARK_TEMPLATE);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const saved: TemplateDto[] = templatesByKind.watermark ?? [];

  useEffect(() => {
    if (open) void refreshTemplates("watermark");
  }, [open, refreshTemplates]);

  useEffect(() => {
    if (!open) return;
    const canvas = canvasRef.current;
    if (canvas) drawPreview(canvas, template);
  }, [open, template]);

  const patch = (change: Partial<WatermarkTemplate>) => setTemplate((prev) => ({ ...prev, ...change }));

  const load = (entry: TemplateDto) => {
    try {
      const parsed = JSON.parse(entry.payload_json) as Partial<WatermarkTemplate>;
      setTemplate({ ...DEFAULT_WATERMARK_TEMPLATE, ...parsed });
      setError(null);
    } catch (err) {
      setError(`Vorlage „${entry.name}" ist beschädigt: ${String(err)}`);
    }
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Wasserzeichen-Vorlagen"
      className="flex max-h-[85vh] w-[50rem] max-w-[94vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="text-sm font-semibold text-text-primary">Wasserzeichen-Vorlagen</h2>
          <p className="mt-0.5 text-xs text-text-muted">
            Größe und Rand in Prozent der kürzeren Bildkante — dieselbe Vorlage sieht auf einem Web- und einem Druckexport
            gleich aus.
          </p>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        <div className="grid min-h-0 flex-1 grid-cols-[1fr_auto] gap-4 overflow-hidden">
          <div className="flex min-h-0 flex-col gap-2 overflow-y-auto pr-1">
            <label className="flex items-center gap-2 text-xs text-text-secondary">
              <span className="w-28 shrink-0">Art</span>
              <select
                value={template.mode}
                onChange={(event) => patch({ mode: event.target.value as WatermarkTemplate["mode"] })}
                aria-label="Art des Wasserzeichens"
                className="rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
              >
                <option value="text">Text</option>
                <option value="image">Bild</option>
              </select>
            </label>

            {template.mode === "text" ? (
              <>
                <label className="flex items-center gap-2 text-xs text-text-secondary">
                  <span className="w-28 shrink-0">Text</span>
                  <input
                    type="text"
                    value={template.text}
                    onChange={(event) => patch({ text: event.target.value })}
                    aria-label="Wasserzeichen-Text"
                    className="flex-1 rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
                  />
                </label>
                <div className="flex items-center gap-2 text-xs text-text-secondary">
                  <span className="w-28 shrink-0">Schriftdatei</span>
                  <input
                    type="text"
                    readOnly
                    value={template.fontPath}
                    aria-label="Schriftdatei"
                    placeholder="keine gewählt — ohne sie kann kein Text gezeichnet werden"
                    className="min-w-0 flex-1 rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
                  />
                  <button
                    type="button"
                    onClick={async () => {
                      const path = await pickFilePath("Schriftdatei", ["ttf", "otf"]);
                      if (path) patch({ fontPath: path });
                    }}
                    className="apx-btn-liquid shrink-0 rounded border border-border px-2 py-1 text-xs text-text-secondary"
                  >
                    Wählen…
                  </button>
                </div>
                <label className="flex items-center gap-2 text-xs text-text-secondary">
                  <span className="w-28 shrink-0">Farbe</span>
                  <input
                    type="color"
                    value={`#${template.color.map((c) => c.toString(16).padStart(2, "0")).join("")}`}
                    onChange={(event) => {
                      const hex = event.target.value;
                      patch({
                        color: [
                          parseInt(hex.slice(1, 3), 16),
                          parseInt(hex.slice(3, 5), 16),
                          parseInt(hex.slice(5, 7), 16),
                        ],
                      });
                    }}
                    aria-label="Farbe des Wasserzeichens"
                    className="h-6 w-12 rounded border border-border bg-bg-base"
                  />
                </label>
              </>
            ) : (
              <div className="flex items-center gap-2 text-xs text-text-secondary">
                <span className="w-28 shrink-0">Bilddatei</span>
                <input
                  type="text"
                  readOnly
                  value={template.imagePath}
                  aria-label="Bilddatei"
                  placeholder="keine gewählt"
                  className="min-w-0 flex-1 rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
                />
                <button
                  type="button"
                  onClick={async () => {
                    const path = await pickFilePath("Bilddatei", ["png", "jpg", "jpeg", "webp"]);
                    if (path) patch({ imagePath: path });
                  }}
                  className="apx-btn-liquid shrink-0 rounded border border-border px-2 py-1 text-xs text-text-secondary"
                >
                  Wählen…
                </button>
              </div>
            )}

            <label className="flex items-center gap-2 text-xs text-text-secondary">
              <span className="w-28 shrink-0">Größe {template.sizePercent} %</span>
              <input
                type="range"
                min={1}
                max={40}
                step={0.5}
                value={template.sizePercent}
                onChange={(event) => patch({ sizePercent: Number(event.target.value) })}
                aria-label="Größe in Prozent"
                className="flex-1"
              />
            </label>
            <label className="flex items-center gap-2 text-xs text-text-secondary">
              <span className="w-28 shrink-0">Deckkraft {Math.round(template.opacity * 100)} %</span>
              <input
                type="range"
                min={0}
                max={1}
                step={0.05}
                value={template.opacity}
                onChange={(event) => patch({ opacity: Number(event.target.value) })}
                aria-label="Deckkraft"
                className="flex-1"
              />
            </label>

            <label className="flex items-center gap-2 text-xs text-text-secondary">
              <input
                type="checkbox"
                checked={template.tile}
                onChange={(event) => patch({ tile: event.target.checked })}
                aria-label="Über das ganze Bild kacheln"
              />
              Über das ganze Bild kacheln
            </label>

            {template.tile ? (
              <>
                <label className="flex items-center gap-2 text-xs text-text-secondary">
                  <span className="w-28 shrink-0">Abstand {template.tileSpacingPercent} %</span>
                  <input
                    type="range"
                    min={0}
                    max={30}
                    step={0.5}
                    value={template.tileSpacingPercent}
                    onChange={(event) => patch({ tileSpacingPercent: Number(event.target.value) })}
                    aria-label="Kachelabstand in Prozent"
                    className="flex-1"
                  />
                </label>
                <label className="flex items-center gap-2 text-xs text-text-secondary">
                  <span className="w-28 shrink-0">Drehung {template.rotationDegrees}°</span>
                  <input
                    type="range"
                    min={-90}
                    max={90}
                    step={5}
                    value={template.rotationDegrees}
                    onChange={(event) => patch({ rotationDegrees: Number(event.target.value) })}
                    aria-label="Drehung in Grad"
                    className="flex-1"
                  />
                </label>
              </>
            ) : (
              <>
                <label className="flex items-center gap-2 text-xs text-text-secondary">
                  <span className="w-28 shrink-0">Position</span>
                  <select
                    value={template.position}
                    onChange={(event) => patch({ position: event.target.value as WatermarkPosition })}
                    aria-label="Position"
                    className="rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
                  >
                    {(Object.keys(POSITION_LABELS) as WatermarkPosition[]).map((key) => (
                      <option key={key} value={key}>
                        {POSITION_LABELS[key]}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex items-center gap-2 text-xs text-text-secondary">
                  <span className="w-28 shrink-0">Rand {template.marginPercent} %</span>
                  <input
                    type="range"
                    min={0}
                    max={20}
                    step={0.5}
                    value={template.marginPercent}
                    onChange={(event) => patch({ marginPercent: Number(event.target.value) })}
                    aria-label="Rand in Prozent"
                    className="flex-1"
                  />
                </label>
              </>
            )}
          </div>

          <div className="flex w-[22rem] shrink-0 flex-col gap-2">
            <canvas
              ref={canvasRef}
              data-testid="watermark-preview"
              aria-label="Vorschau des Wasserzeichens"
              style={{ width: PREVIEW_W, height: PREVIEW_H }}
              className="rounded border border-border"
            />
            <p className="text-[11px] text-text-muted">
              Die Vorschau zeigt Lage, Größe und Kachelung — nicht die echte Schriftart. Die kommt beim Export aus der
              gewählten Schriftdatei.
            </p>

            <h3 className="mt-1 text-xs font-semibold text-text-secondary">Gespeicherte Vorlagen</h3>
            <ul className="flex max-h-32 flex-col gap-1 overflow-y-auto" data-testid="watermark-template-list">
              {saved.length === 0 && <li className="text-[11px] text-text-muted">Noch keine.</li>}
              {saved.map((entry) => (
                <li key={entry.id} className="flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() => load(entry)}
                    className="apx-btn-liquid min-w-0 flex-1 truncate rounded border border-border px-2 py-1 text-left text-[11px] text-text-primary"
                  >
                    {entry.name}
                  </button>
                  <button
                    type="button"
                    onClick={() => void deleteTemplateAction("watermark", entry.id)}
                    aria-label={`Vorlage ${entry.name} löschen`}
                    className="apx-btn-liquid rounded border border-border p-1 text-text-muted"
                  >
                    <Trash2 aria-hidden="true" className="size-3" />
                  </button>
                </li>
              ))}
            </ul>

            <div className="flex gap-1">
              <input
                type="text"
                value={name}
                onChange={(event) => setName(event.target.value)}
                aria-label="Name der Wasserzeichen-Vorlage"
                placeholder="Name der Vorlage"
                className="min-w-0 flex-1 rounded border border-border bg-bg-base px-2 py-1 text-[11px] text-text-primary"
              />
              <button
                type="button"
                onClick={() => {
                  if (!name.trim()) return;
                  void saveTemplateAction("watermark", name.trim(), template);
                  setName("");
                }}
                disabled={!name.trim()}
                aria-label="Wasserzeichen-Vorlage speichern"
                className="apx-btn-liquid flex shrink-0 items-center gap-1 rounded border border-border px-2 py-1 text-[11px] text-text-secondary disabled:opacity-40"
              >
                <Save aria-hidden="true" className="size-3" />
                Speichern
              </button>
            </div>
          </div>
        </div>

        <div className="flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="apx-btn-liquid rounded border border-border px-3 py-1 text-xs text-text-secondary"
          >
            Schließen
          </button>
        </div>
      </div>
    </Dialog>
  );
}
