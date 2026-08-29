import { useEffect, useRef, useState } from "react";

/**
 * Lädt ein Bild über `fetch()` + `createImageBitmap()` statt über ein
 * einfaches `<img src>` — nur `fetch()` erlaubt einen `AbortController`,
 * über den ein Wechsel zu einem neuen Bild die noch laufende Anfrage für
 * das alte abbricht (siehe `crates/apx-app/src/protocol/mod.rs`: ein
 * bereits laufender Dekodier-Thread im Backend lässt sich nicht
 * unterbrechen, das Frontend verhindert aber wenigstens, ein veraltetes
 * Ergebnis noch zu verarbeiten).
 *
 * Schließt jedes selbst erzeugte `ImageBitmap`, sobald es ersetzt wird
 * oder die Komponente unmountet — sonst entsteht das in
 * `PHASE1_PROMPT.md` Abschnitt 10 genannte Speicherleck.
 */
export function useImageBitmap(url: string | null): ImageBitmap | null {
  const [bitmap, setBitmap] = useState<ImageBitmap | null>(null);
  const ownedBitmap = useRef<ImageBitmap | null>(null);

  useEffect(() => {
    if (!url) {
      setBitmap(null);
      return;
    }

    const controller = new AbortController();

    void (async () => {
      try {
        const response = await fetch(url, { signal: controller.signal });
        if (!response.ok) {
          throw new Error(`HTTP ${response.status} für ${url}`);
        }
        const blob = await response.blob();
        const decoded = await createImageBitmap(blob);

        if (controller.signal.aborted) {
          decoded.close();
          return;
        }

        ownedBitmap.current?.close();
        ownedBitmap.current = decoded;
        setBitmap(decoded);
      } catch (err) {
        if ((err as DOMException).name !== "AbortError") {
          console.error("Bild konnte nicht geladen werden:", url, err);
        }
      }
    })();

    return () => {
      controller.abort();
    };
  }, [url]);

  // Beim endgültigen Unmount das zuletzt gehaltene Bitmap freigeben.
  useEffect(() => {
    return () => {
      ownedBitmap.current?.close();
      ownedBitmap.current = null;
    };
  }, []);

  return bitmap;
}
