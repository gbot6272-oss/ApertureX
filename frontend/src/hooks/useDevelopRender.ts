import { useEffect, useState } from "react";

import { developUrl } from "../lib/media";

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

/**
 * Rendert `photoId` live über die `develop/...`-Route mit dem aktuellen
 * `edlJson`-Zustand — analog zu `useImageBitmap`, aber für das rohe
 * RGBA8-Antwortformat (kein `createImageBitmap`, das erwartet ein
 * dekodierbares Bildformat). Bricht eine noch laufende Anfrage ab, sobald
 * eine neuere für denselben Aufruf unterwegs ist (siehe `useImageBitmap`s
 * Moduldoku für die Begründung — dieselbe Backend-Einschränkung gilt
 * hier).
 */
export function useDevelopRender(photoId: string | null, edlJson: string | null, maxEdge: number | undefined): DevelopFrame | null {
  const [frame, setFrame] = useState<DevelopFrame | null>(null);

  useEffect(() => {
    if (!photoId || !edlJson) {
      setFrame(null);
      return;
    }

    const controller = new AbortController();
    const url = developUrl(photoId, edlJson, maxEdge);

    void (async () => {
      try {
        const response = await fetch(url, { signal: controller.signal });
        if (!response.ok) {
          throw new Error(`HTTP ${response.status} für ${url}`);
        }
        const buffer = await response.arrayBuffer();
        if (controller.signal.aborted) return;
        setFrame(parseFrame(buffer));
      } catch (err) {
        if ((err as DOMException).name !== "AbortError") {
          console.error("Entwickeln-Rendering fehlgeschlagen:", url, err);
        }
      }
    })();

    return () => {
      controller.abort();
    };
  }, [photoId, edlJson, maxEdge]);

  return frame;
}
