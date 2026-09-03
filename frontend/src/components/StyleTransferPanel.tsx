import { DevelopSlider } from "./DevelopSlider";
import { STYLE_TRANSFER_SLIDER_SPECS, STYLE_TRANSFER_STYLES } from "../lib/edl";
import { useAppStore } from "../store";

/**
 * KI-Stiltransfer zwischen Fotos (Phase 14 Schritt 9, siehe
 * `DECISIONS.md` ADR-0041 Nachtrag IX) — Lightroom hat dafür kein
 * Äquivalent. Anders als ursprünglich erhofft (ein *beliebiges*
 * Referenzfoto als Stilvorlage) bewusst auf fünf real lizenzierte feste
 * `fast_neural_style`-Netze beschränkt (MIT, `onnx/models`,
 * `STYLE_TRANSFER_STYLES`) — ein lizenzklares Modell für echte
 * beliebige Referenzbild-Übertragung wurde in Schritt 0 real gesucht,
 * aber nicht gefunden (siehe Moduldoku).
 *
 * Ablauf: pro Stil einzeln das opt-in ~6,7-MB-Modell herunterladen,
 * dann "Stilisieren" mit dem gewählten Stil — der "Betrag"-Regler
 * blendet anschließend linear zwischen dem unveränderten Foto und dem
 * vollen Stiltransfer-Ergebnis.
 */
export function StyleTransferPanel() {
  const developPhotoId = useAppStore((s) => s.developPhotoId);
  const styleTransfer = useAppStore((s) => s.developEdl.style_transfer);
  const setStyleTransferAmount = useAppStore((s) => s.setStyleTransferAmount);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);
  const styleTransferStylizing = useAppStore((s) => s.styleTransferStylizing);
  const stylizePhotoWithStyle = useAppStore((s) => s.stylizePhotoWithStyle);
  const aiSettings = useAppStore((s) => s.aiSettings);
  const styleTransferModelDownloading = useAppStore((s) => s.styleTransferModelDownloading);
  const downloadStyleTransferModel = useAppStore((s) => s.downloadStyleTransferModel);
  const clearStyleTransferModelPath = useAppStore((s) => s.clearStyleTransferModelPath);

  if (!developPhotoId) return null;

  const modelPaths = aiSettings?.style_transfer_model_paths ?? {};

  return (
    <div className="flex flex-col gap-2">
      <ul className="flex flex-col gap-1">
        {STYLE_TRANSFER_STYLES.map((style) => {
          const hasModel = !!modelPaths[style.id];
          const downloading = styleTransferModelDownloading === style.id;
          return (
            <li key={style.id} className="flex items-center justify-between gap-2 rounded border border-border px-2 py-1 text-xs">
              <span className="text-text-secondary">{style.label}</span>
              {hasModel ? (
                <span className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => void stylizePhotoWithStyle(style.id)}
                    disabled={styleTransferStylizing}
                    className="rounded border border-accent bg-accent/10 px-2 py-0.5 text-accent disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {styleTransferStylizing ? "Stilisiert…" : "Stilisieren"}
                  </button>
                  <button
                    type="button"
                    onClick={() => void clearStyleTransferModelPath(style.id)}
                    className="text-text-muted underline hover:text-danger"
                  >
                    Entfernen
                  </button>
                </span>
              ) : (
                <button
                  type="button"
                  disabled={downloading}
                  onClick={() => void downloadStyleTransferModel(style.id)}
                  className="text-accent underline disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {downloading ? "Lädt herunter…" : "Herunterladen"}
                </button>
              )}
            </li>
          );
        })}
      </ul>

      {!styleTransfer.patch && <p className="text-xs text-text-muted">Noch kein Stiltransfer-Ergebnis berechnet.</p>}

      {STYLE_TRANSFER_SLIDER_SPECS.map((spec) => (
        <DevelopSlider
          key={spec.key}
          spec={spec}
          value={styleTransfer.amount * 100}
          onChange={(value) => setStyleTransferAmount(value / 100)}
          onCommit={() => void commitDevelopEdit()}
        />
      ))}
    </div>
  );
}
