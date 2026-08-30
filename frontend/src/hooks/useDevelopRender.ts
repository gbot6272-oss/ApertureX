import { useEffect, useState } from "react";

import { developUrl } from "../lib/media";
import { useAppStore } from "../store";

export interface DevelopFrame {
  width: number;
  height: number;
  /** Interleaved RGBA8, Länge = `width * height * 4`. */
  pixels: Uint8Array;
}

/** Parst das in `crates/apx-app/src/protocol/mod.rs` dokumentierte
 * Antwortformat: die ersten 8 Bytes sind Breite/Höhe als `u32`
 * little-endian, danach folgt rohes RGBA8. */
function parseFrame(buffer: ArrayBuffer): DevelopFrame {
  const view = new DataView(buffer);
  const width = view.getUint32(0, true);
  const height = view.getUint32(4, true);
  const pixels = new Uint8Array(buffer, 8);
  return { width, height, pixels };
}

/** Gemeinsamer, entprellter Fetch-Kern für `useDevelopRender` und
 * `useDevelopPreviewThumbnail` — identische Anfrage-/Abbruch-/
 * Entprellungslogik, nur der optionale `onLatency`-Callback
 * unterscheidet sie (siehe `useDevelopRender`s Moduldoku für die
 * Begründung der `requestAnimationFrame`-Entprellung). */
function useDevelopFrameInternal(
  photoId: string | null,
  edlJson: string | null,
  maxEdge: number | undefined,
  onLatency?: (ms: number) => void,
): DevelopFrame | null {
  const [frame, setFrame] = useState<DevelopFrame | null>(null);

  useEffect(() => {
    if (!photoId || !edlJson) {
      setFrame(null);
      return;
    }

    const controller = new AbortController();
    const url = developUrl(photoId, edlJson, maxEdge);

    const rafId = requestAnimationFrame(() => {
      const startedAt = performance.now();
      void (async () => {
        try {
          const response = await fetch(url, { signal: controller.signal });
          if (!response.ok) {
            throw new Error(`HTTP ${response.status} für ${url}`);
          }
          const buffer = await response.arrayBuffer();
          if (controller.signal.aborted) return;
          setFrame(parseFrame(buffer));
          onLatency?.(performance.now() - startedAt);
        } catch (err) {
          if ((err as DOMException).name !== "AbortError") {
            console.error("Entwickeln-Rendering fehlgeschlagen:", url, err);
          }
        }
      })();
    });

    return () => {
      cancelAnimationFrame(rafId);
      controller.abort();
    };
  }, [photoId, edlJson, maxEdge, onLatency]);

  return frame;
}

/**
 * Rendert `photoId` live über die `develop/...`-Route mit dem aktuellen
 * `edlJson`-Zustand — analog zu `useImageBitmap`, aber für das rohe
 * RGBA8-Antwortformat (kein `createImageBitmap`, das erwartet ein
 * dekodierbares Bildformat). Bricht eine noch laufende Anfrage ab, sobald
 * eine neuere für denselben Aufruf unterwegs ist (siehe `useImageBitmap`s
 * Moduldoku für die Begründung — dieselbe Backend-Einschränkung gilt
 * hier).
 *
 * **Entprellung (`PLAN.md` Phase 2 Schritt 7):** eine neue Anfrage wird
 * nicht sofort bei jeder `edlJson`-Änderung losgeschickt, sondern erst im
 * nächsten `requestAnimationFrame` — das koppelt die Anfragerate an den
 * tatsächlichen Bild-Rhythmus des Geräts (typischerweise 60 Hz) statt an
 * eine feste Millisekundenzahl, die auf sehr schnellen oder sehr
 * langsamen Bildschirmen falsch wäre. Mehrere `edlJson`-Änderungen
 * innerhalb desselben Frames lösen dadurch nur eine Anfrage aus (die
 * letzte gewinnt).
 *
 * Misst außerdem die Ende-zu-Ende-Antwortzeit (`performance.now()` vor
 * `fetch()` bis zur fertig geparsten Antwort) und schreibt sie in
 * `developLastLatencyMs` — sichtbar in `DevelopPanel`, siehe
 * `DECISIONS.md`/`PLAN.md` Phase 2 Schritt 7 zur ehrlichen
 * Performance-Dokumentation (diese Zahl misst IPC + Dekodierung/Rendern,
 * nicht das Neuzeichnen im Browser selbst).
 */
export function useDevelopRender(photoId: string | null, edlJson: string | null, maxEdge: number | undefined): DevelopFrame | null {
  const setDevelopLatencyMs = useAppStore((s) => s.setDevelopLatencyMs);
  return useDevelopFrameInternal(photoId, edlJson, maxEdge, setDevelopLatencyMs);
}

/**
 * Wie `useDevelopRender`, aber ohne die globale `developLastLatencyMs`-
 * Anzeige zu berühren — für die kleinen Preset-Thumbnails (Phase 5
 * Schritt 6, `SPEC.md` §3.5: „Live-Vorschau … als Thumbnail des
 * aktuellen Bildes in der Preset-Liste"), von denen potenziell mehrere
 * gleichzeitig auf dem Bildschirm sichtbar sind — die für den
 * Haupt-Viewer gedachte Performance-Anzeige (`PLAN.md` Phase 2 Schritt 7)
 * soll nicht mit jedem kleinen Thumbnail-Rendering überschrieben werden.
 */
export function useDevelopPreviewThumbnail(photoId: string | null, edlJson: string | null, maxEdge: number | undefined): DevelopFrame | null {
  return useDevelopFrameInternal(photoId, edlJson, maxEdge);
}
