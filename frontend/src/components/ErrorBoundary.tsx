import { Component, type ErrorInfo, type ReactNode } from "react";

/**
 * Fängt Render-Fehler ab, statt die Anwendung verschwinden zu lassen
 * (siehe `DECISIONS.md` ADR-0068).
 *
 * **Warum es das gibt.** Im gesamten Frontend gab es bis Phase 34 keine
 * einzige Fehlergrenze. React hängt bei einem Fehler im Render den
 * kompletten Baum ab — die Folge ist ein leeres, weißes Fenster ohne
 * Kopfleiste, ohne Escape, ohne Hinweis. Genau so wurde es gemeldet:
 * „einfach ein weißer Bildschirm und keine Möglichkeit es wegzumachen".
 * Ein abgestürztes Panel darf die App nicht mitnehmen, und wenn doch
 * etwas abstürzt, muss man sehen, *was*.
 *
 * **Zwei Ebenen.** `scope="app"` (in `main.tsx` um die ganze App) ist das
 * letzte Netz und zeigt eine ganzseitige Meldung mit Neu-laden-Knopf.
 * `scope="region"` (um einzelne Bereiche) ersetzt nur diesen Bereich
 * durch eine kleine Meldung mit „Nochmal versuchen" — der Rest der
 * Oberfläche bleibt bedienbar.
 *
 * Die Fehlermeldung wird bewusst im Klartext angezeigt und ist
 * kopierbar: ohne sie ist ein solcher Absturz aus einem Fehlerbericht
 * heraus nicht nachvollziehbar.
 */
interface Props {
  children: ReactNode;
  scope: "app" | "region";
  /** Erscheint in der Meldung, damit klar ist, welcher Teil betroffen ist. */
  label?: string;
}

interface State {
  error: Error | null;
  componentStack: string | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, componentStack: null };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // Zusätzlich zur Anzeige in die Konsole — die Entwicklerwerkzeuge
    // sind im Tauri-Fenster erreichbar und zeigen dort den vollen Stack.
    console.error("Unbehandelter Render-Fehler:", error, info.componentStack);
    this.setState({ componentStack: info.componentStack ?? null });
  }

  private reset = (): void => {
    this.setState({ error: null, componentStack: null });
  };

  private copyDetails = (): void => {
    const { error, componentStack } = this.state;
    const text = [error?.message, error?.stack, componentStack].filter(Boolean).join("\n\n");
    void navigator.clipboard?.writeText(text);
  };

  render(): ReactNode {
    const { error } = this.state;
    if (!error) return this.props.children;

    const { scope, label } = this.props;
    const title = label ? `${label} konnte nicht angezeigt werden` : "Etwas ist schiefgelaufen";

    const details = (
      <>
        <p className="text-sm font-medium text-text-primary">{title}</p>
        <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words rounded border border-border bg-bg-base p-2 text-[11px] text-text-secondary">
          {error.message || String(error)}
        </pre>
      </>
    );

    if (scope === "region") {
      return (
        <div
          role="alert"
          data-testid="error-boundary-region"
          className="m-2 flex flex-col gap-2 rounded border border-danger/50 bg-bg-panel p-3"
        >
          {details}
          <div className="flex gap-2">
            <button
              type="button"
              onClick={this.reset}
              className="apx-btn-liquid rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent"
            >
              Nochmal versuchen
            </button>
            <button
              type="button"
              onClick={this.copyDetails}
              className="apx-btn-liquid rounded border border-border px-2 py-1 text-xs text-text-secondary hover:border-accent hover:text-text-primary"
            >
              Details kopieren
            </button>
          </div>
        </div>
      );
    }

    return (
      <div
        role="alert"
        data-testid="error-boundary-app"
        className="fixed inset-0 z-[100] flex items-center justify-center bg-bg-base p-6"
      >
        <div className="flex w-full max-w-lg flex-col gap-3 rounded border border-danger/50 bg-bg-panel p-4 shadow-xl">
          {details}
          <p className="text-xs text-text-muted">
            Die Bearbeitungen im Katalog sind davon nicht betroffen — sie liegen in der Katalogdatei, nicht in
            dieser Ansicht.
          </p>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => window.location.reload()}
              className="apx-btn-liquid rounded border border-accent bg-accent/10 px-3 py-1 text-sm text-accent"
            >
              Neu laden
            </button>
            <button
              type="button"
              onClick={this.reset}
              className="apx-btn-liquid rounded border border-border px-3 py-1 text-sm text-text-secondary hover:border-accent hover:text-text-primary"
            >
              Weiter ohne Neuladen
            </button>
            <button
              type="button"
              onClick={this.copyDetails}
              className="apx-btn-liquid rounded border border-border px-3 py-1 text-sm text-text-secondary hover:border-accent hover:text-text-primary"
            >
              Details kopieren
            </button>
          </div>
        </div>
      </div>
    );
  }
}
