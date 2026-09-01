import { useEffect, useState } from "react";

import { imageUrl } from "./lib/media";
import type { PhotoDto } from "./lib/tauri";
import { getPhoto } from "./lib/tauri";

/**
 * Sekundäres Display (Phase 9 Schritt 3, siehe `PLAN.md`/`DECISIONS.md`
 * ADR-0035) — ein unabhängiges zweites Tauri-Fenster, das nur ein
 * einzelnes Foto großformatig zeigt (z. B. auf einem zweiten Monitor als
 * Kunden-/Vorführ-Ansicht). Öffnet über `store.openSecondaryDisplay` als
 * eigenes `WebviewWindow` mit `?secondaryPhoto=<id>` in der URL — `main.tsx`
 * rendert dann diese Komponente statt der vollen `<App/>`.
 *
 * **Bewusste Vereinfachung**: statisches Bild über den bestehenden
 * `image/<id>/full`-Protokoll-Handler (kein Live-Renderer/WebGL wie im
 * Haupt-Viewer, kein automatisches Nachziehen bei Entwickeln-Änderungen
 * im Hauptfenster) — für eine reine Vorführ-Ansicht ausreichend, echte
 * Live-Synchronisation zwischen zwei Fenstern wäre ein State-Sync-Aufwand
 * außerhalb des Umfangs dieses Schritts.
 */
export function SecondaryDisplay({ photoId }: { photoId: string }) {
  const [photo, setPhoto] = useState<PhotoDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void getPhoto(photoId)
      .then((p) => {
        if (!cancelled) setPhoto(p);
      })
      .catch((err) => {
        if (!cancelled) setError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [photoId]);

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-black">
      {error && <p className="text-sm text-danger">{error}</p>}
      {!error && photo && <img src={imageUrl(photo.id)} alt={photo.filename} className="max-h-full max-w-full object-contain" />}
    </div>
  );
}
