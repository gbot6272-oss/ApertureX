import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { SecondaryDisplay } from "./SecondaryDisplay";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./index.css";

const rootElement = document.getElementById("root");
if (!rootElement) {
  throw new Error("Root-Element '#root' fehlt in index.html");
}

// Sekundäres Display (Phase 9 Schritt 3, siehe `store.openSecondaryDisplay`):
// ein per `?secondaryPhoto=<id>` geöffnetes zweites Fenster rendert nur
// `SecondaryDisplay` statt der vollen App — kein eigener Router nötig für
// diesen einen zusätzlichen Fall.
const secondaryPhotoId = new URLSearchParams(window.location.search).get("secondaryPhoto");

// Die aeussere Fehlergrenze ist das letzte Netz: ohne sie haengt React
// bei jedem Render-Fehler den ganzen Baum ab und zurueck bleibt ein
// leeres weisses Fenster ohne Bedienelemente (siehe
// `components/ErrorBoundary.tsx` und `DECISIONS.md` ADR-0068).
ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <ErrorBoundary scope="app">
      {secondaryPhotoId ? <SecondaryDisplay photoId={secondaryPhotoId} /> : <App />}
    </ErrorBoundary>
  </React.StrictMode>,
);
