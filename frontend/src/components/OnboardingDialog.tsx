import { useRef } from "react";

import { useFocusTrap } from "../lib/a11y";
import { useT } from "../lib/i18n";

interface OnboardingDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * Onboarding (Phase 10 Schritt 9, siehe SPEC.md §5 Phase 10). Erscheint
 * automatisch einmalig (`uiSettings.onboarding_seen`, siehe App.tsx),
 * jederzeit erneut über die Befehlspalette ("Erste Schritte anzeigen")
 * aufrufbar. Kurze Einführung statt einer mehrseitigen Tour — verweist
 * für Details auf das bereits vorhandene Cheatsheet-Overlay (`?`) statt
 * dessen Inhalt zu duplizieren.
 */
export function OnboardingDialog({ open, onClose }: OnboardingDialogProps) {
  const t = useT();
  const dialogRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dialogRef, open);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={t("onboarding.title")}
        className="flex max-h-[80vh] w-full max-w-lg flex-col rounded-lg border border-border bg-bg-raised shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-sm font-semibold">{t("onboarding.title")}</h2>
          <button type="button" onClick={onClose} className="text-text-secondary hover:text-text-primary" aria-label={t("settings.close")}>
            ✕
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-4 text-xs">
          <ul className="flex flex-col gap-3">
            <li>
              <p className="font-semibold text-text-primary">{t("onboarding.layout.title")}</p>
              <p className="text-text-secondary">{t("onboarding.layout.body")}</p>
            </li>
            <li>
              <p className="font-semibold text-text-primary">{t("onboarding.import.title")}</p>
              <p className="text-text-secondary">{t("onboarding.import.body")}</p>
            </li>
            <li>
              <p className="font-semibold text-text-primary">{t("onboarding.develop.title")}</p>
              <p className="text-text-secondary">{t("onboarding.develop.body")}</p>
            </li>
            <li>
              <p className="font-semibold text-text-primary">{t("onboarding.palette.title")}</p>
              <p className="text-text-secondary">{t("onboarding.palette.body")}</p>
            </li>
            <li>
              <p className="font-semibold text-text-primary">{t("onboarding.shortcuts.title")}</p>
              <p className="text-text-secondary">{t("onboarding.shortcuts.body")}</p>
            </li>
          </ul>
        </div>

        <div className="border-t border-border px-4 py-3">
          <button type="button" onClick={onClose} className="rounded border border-accent bg-accent/10 px-3 py-1.5 text-sm text-accent hover:bg-accent/20">
            {t("onboarding.start")}
          </button>
        </div>
      </div>
    </div>
  );
}
