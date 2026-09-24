import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ErrorBoundary } from "./ErrorBoundary";

/**
 * Prüft die Fehlergrenze aus ADR-0068 an ihrem einzigen Zweck: ein
 * Render-Fehler darf nicht mehr zum leeren weißen Fenster führen.
 *
 * Bewusst ohne Testing-Library — das Projekt hält sich bei Vitest an
 * reine Logiktests ohne zusätzliche Abhängigkeit (siehe `THIRD_PARTY.md`);
 * `react-dom/client` liegt ohnehin vor und reicht hier vollkommen.
 */

function Boom(): never {
  throw new Error("Kaputt im Render");
}

describe("ErrorBoundary", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    // React protokolliert den gefangenen Fehler zusätzlich selbst —
    // das soll die Testausgabe nicht fluten.
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
  });

  it("zeigt statt eines leeren Fensters eine Meldung mit der Fehlerursache", () => {
    act(() => {
      root.render(
        <ErrorBoundary scope="app">
          <Boom />
        </ErrorBoundary>,
      );
    });

    const fallback = container.querySelector('[data-testid="error-boundary-app"]');
    expect(fallback).not.toBeNull();
    expect(container.textContent).toContain("Kaputt im Render");
    // Der entscheidende Punkt des Fundes: es gibt einen Ausweg.
    expect(container.textContent).toContain("Neu laden");
  });

  it("ersetzt im Bereichsmodus nur den Bereich und nennt ihn beim Namen", () => {
    act(() => {
      root.render(
        <ErrorBoundary scope="region" label="Das Entwickeln-Panel">
          <Boom />
        </ErrorBoundary>,
      );
    });

    expect(container.querySelector('[data-testid="error-boundary-region"]')).not.toBeNull();
    expect(container.textContent).toContain("Das Entwickeln-Panel konnte nicht angezeigt werden");
    expect(container.textContent).toContain("Nochmal versuchen");
  });

  it("reicht fehlerfreie Kinder unverändert durch", () => {
    act(() => {
      root.render(
        <ErrorBoundary scope="app">
          <p>alles gut</p>
        </ErrorBoundary>,
      );
    });

    expect(container.textContent).toBe("alles gut");
    expect(container.querySelector('[data-testid="error-boundary-app"]')).toBeNull();
  });
});
