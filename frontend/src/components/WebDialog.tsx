import { useState } from "react";

import { useT } from "../lib/i18n";
import { selectFolderDialog, type GalleryTheme, type WebGalleryOptions, type WebUploadOptions } from "../lib/tauri";
import { useAppStore } from "../store";

interface WebDialogProps {
  open: boolean;
  photoIds: string[];
  onClose: () => void;
}

/**
 * Web-Galerie-Dialog (Phase 8 Schritt 6, siehe `PLAN.md`/`apx_export::web`s
 * Moduldoku). Rendert die ausgewählten Fotos zu einer statischen
 * HTML-Galerie in einem Zielordner und lädt sie optional per FTP/SFTP auf
 * einen Server hoch.
 */
export function WebDialog({ open, photoIds, onClose }: WebDialogProps) {
  const t = useT();
  const webExportRunning = useAppStore((s) => s.webExportRunning);
  const webExportError = useAppStore((s) => s.webExportError);
  const webExportOutcome = useAppStore((s) => s.webExportOutcome);
  const exportWebGallery = useAppStore((s) => s.exportWebGallery);

  const THEME_LABELS: Record<GalleryTheme, string> = {
    light: t("webDialog.themeLight"),
    dark: t("webDialog.themeDark"),
    minimal: t("webDialog.themeMinimal"),
  };

  const [title, setTitle] = useState("");
  const [theme, setTheme] = useState<GalleryTheme>("light");
  const [uploadEnabled, setUploadEnabled] = useState(false);
  const [protocol, setProtocol] = useState<"ftp" | "sftp">("ftp");
  const [host, setHost] = useState("");
  const [port, setPort] = useState(21);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [remoteDir, setRemoteDir] = useState("");

  if (!open) return null;

  async function handleExport() {
    const destDir = await selectFolderDialog();
    if (!destDir) return;
    const upload: WebUploadOptions | undefined = uploadEnabled
      ? { protocol, host, port, username, password, remoteDir: remoteDir || undefined }
      : undefined;
    const options: WebGalleryOptions = {
      title: title || t("webDialog.defaultTitle"),
      theme,
      upload,
    };
    await exportWebGallery(photoIds, destDir, options);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-16" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="max-h-[85vh] w-full max-w-md overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">{t("webDialog.title")}</h2>
        <p className="mb-3 text-xs text-text-muted">
          {t("webDialog.photoCount", { count: photoIds.length, plural: photoIds.length === 1 ? "" : "s" })}
        </p>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          {t("webDialog.galleryTitle")}
          <input type="text" value={title} onChange={(e) => setTitle(e.target.value)} placeholder={t("webDialog.defaultTitle")} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm" />
        </label>

        <label className="mb-3 flex flex-col gap-1 text-xs text-text-secondary">
          {t("webDialog.theme")}
          <select value={theme} onChange={(e) => setTheme(e.target.value as GalleryTheme)} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
            {(Object.keys(THEME_LABELS) as GalleryTheme[]).map((key) => (
              <option key={key} value={key}>
                {THEME_LABELS[key]}
              </option>
            ))}
          </select>
        </label>

        <label className="mb-3 flex items-center gap-2 text-xs text-text-secondary">
          <input type="checkbox" checked={uploadEnabled} onChange={(e) => setUploadEnabled(e.target.checked)} />
          {t("webDialog.uploadDirectly")}
        </label>

        {uploadEnabled && (
          <div className="mb-3 flex flex-col gap-2 rounded border border-border p-2">
            <label className="flex flex-col gap-1 text-xs text-text-secondary">
              {t("webDialog.protocol")}
              <select value={protocol} onChange={(e) => setProtocol(e.target.value as "ftp" | "sftp")} className="rounded border border-border bg-bg-panel px-2 py-1 text-sm">
                <option value="ftp">FTP</option>
                <option value="sftp">SFTP</option>
              </select>
            </label>
            <div className="flex gap-2">
              <label className="flex flex-[2] flex-col gap-1 text-xs text-text-secondary">
                {t("webDialog.host")}
                <input type="text" value={host} onChange={(e) => setHost(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1" />
              </label>
              <label className="flex flex-1 flex-col gap-1 text-xs text-text-secondary">
                {t("webDialog.port")}
                <input type="number" value={port} onChange={(e) => setPort(Number(e.target.value))} className="rounded border border-border bg-bg-panel px-2 py-1" />
              </label>
            </div>
            <label className="flex flex-col gap-1 text-xs text-text-secondary">
              {t("webDialog.username")}
              <input type="text" value={username} onChange={(e) => setUsername(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1" />
            </label>
            <label className="flex flex-col gap-1 text-xs text-text-secondary">
              {t("webDialog.password")}
              <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1" />
            </label>
            <label className="flex flex-col gap-1 text-xs text-text-secondary">
              {t("webDialog.remoteDir")}
              <input type="text" value={remoteDir} onChange={(e) => setRemoteDir(e.target.value)} className="rounded border border-border bg-bg-panel px-2 py-1" />
            </label>
          </div>
        )}

        {webExportError && <p className="mb-2 text-xs text-danger">{t("webDialog.error", { message: webExportError })}</p>}
        {!webExportRunning && webExportOutcome && (
          <p className="mb-2 text-xs text-text-secondary">
            {t("webDialog.savedOutcome", { destDir: webExportOutcome.dest_dir, photoCount: webExportOutcome.photo_count })}
            {webExportOutcome.uploaded_count !== null && ` — ${t("webDialog.uploadedCount", { count: webExportOutcome.uploaded_count })}`}
          </p>
        )}

        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs hover:border-accent">
            {t("webDialog.close")}
          </button>
          <button
            type="button"
            onClick={() => void handleExport()}
            disabled={photoIds.length === 0 || webExportRunning}
            className="rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {webExportRunning ? t("webDialog.generating") : t("webDialog.generateGallery")}
          </button>
        </div>
      </div>
    </div>
  );
}
