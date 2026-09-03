import { pickFilePath } from "../lib/tauri";
import { useAppStore } from "../store";

/** Himmelsaustausch mit grober automatischer Neubelichtung (Phase 14 Schritt 10). */
export function SkyReplacePanel() {
  const developPhotoId = useAppStore((s) => s.developPhotoId);
  const skyReplace = useAppStore((s) => s.developEdl.sky_replace);
  const skyReplacing = useAppStore((s) => s.skyReplacing);
  const replaceSkyForCurrentPhoto = useAppStore((s) => s.replaceSkyForCurrentPhoto);
  const clearSkyReplace = useAppStore((s) => s.clearSkyReplace);

  if (!developPhotoId) return null;

  async function pickAndReplace() {
    const path = await pickFilePath("Bilder", ["jpg", "jpeg", "png"]);
    if (path) void replaceSkyForCurrentPhoto(path);
  }

  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        onClick={() => void pickAndReplace()}
        disabled={skyReplacing}
        className="rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
      >
        {skyReplacing ? "Ersetzt…" : "Himmel-Foto wählen und ersetzen…"}
      </button>
      {skyReplace ? (
        <button type="button" onClick={clearSkyReplace} className="text-left text-xs text-text-muted underline hover:text-danger">
          Entfernen
        </button>
      ) : (
        <p className="text-xs text-text-muted">Noch kein Himmel ersetzt.</p>
      )}
    </div>
  );
}
