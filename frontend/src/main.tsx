import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { SecondaryDisplay } from "./SecondaryDisplay";
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

ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>{secondaryPhotoId ? <SecondaryDisplay photoId={secondaryPhotoId} /> : <App />}</React.StrictMode>,
);
