import L from "leaflet";
import "leaflet/dist/leaflet.css";
import { useEffect, useRef } from "react";

import { pickFilePath, reverseGeocodeLocation, type PhotoDto } from "../lib/tauri";
import { useAppStore } from "../store";

/** Farbiger Kreis-Marker statt Leaflets Standard-Pin-Bildern — vermeidet
 * das bekannte Bundler-Problem mit Leaflets Standard-Icon-Pfaden (siehe
 * z. B. https://github.com/Leaflet/Leaflet/issues/4968) ganz, ohne
 * zusätzliche Asset-Konfiguration. */
function dotIcon(color: string): L.DivIcon {
  return L.divIcon({
    className: "",
    html: `<div style="width:14px;height:14px;border-radius:50%;background:${color};border:2px solid white;box-shadow:0 0 2px rgba(0,0,0,0.6)"></div>`,
    iconSize: [14, 14],
    iconAnchor: [7, 7],
  });
}

const PHOTO_ICON = dotIcon("#3b82f6");
const SELECTED_ICON = dotIcon("#f59e0b");

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function popupHtmlFor(photo: PhotoDto, locationName?: string, countryCode?: string): string {
  const location = locationName ? `${locationName}${countryCode ? `, ${countryCode}` : ""}` : "Ort wird ermittelt…";
  return `<strong>${escapeHtml(photo.filename)}</strong><br/>${escapeHtml(location)}`;
}

/**
 * Kartenansicht (Phase 8 Schritt 7, siehe `PLAN.md`/`apx_export::map`s
 * Moduldoku). Zeigt alle geotaggten Fotos als Marker auf einer
 * OpenStreetMap-Kachelkarte (Leaflet, einzige Netzwerk-Abhängigkeit
 * dieser Phase), verbindet sie nach Aufnahmezeit als Reiserouten-Linie,
 * kann einen importierten GPX-Track überlagern und erlaubt, Fotos ohne
 * EXIF-GPS per Klick auf die Karte zu platzieren.
 *
 * Bewusst mit Leaflet direkt statt `react-leaflet` (kein zusätzlicher
 * Adapter-Layer nötig, ein `useEffect` mit einer imperativen Leaflet-
 * Instanz reicht für diesen einen View).
 */
export function MapView() {
  const containerRef = useRef<HTMLDivElement>(null);
  const mapRef = useRef<L.Map | null>(null);
  const markersLayerRef = useRef<L.LayerGroup | null>(null);
  const routeLayerRef = useRef<L.Polyline | null>(null);
  const gpxLayerRef = useRef<L.Polyline | null>(null);

  const geotaggedPhotos = useAppStore((s) => s.geotaggedPhotos);
  const refreshGeotaggedPhotos = useAppStore((s) => s.refreshGeotaggedPhotos);
  const selectPhoto = useAppStore((s) => s.selectPhoto);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const gpxTrack = useAppStore((s) => s.gpxTrack);
  const placingGpsForPhotoId = useAppStore((s) => s.placingGpsForPhotoId);
  const startPlacingGps = useAppStore((s) => s.startPlacingGps);
  const cancelPlacingGps = useAppStore((s) => s.cancelPlacingGps);
  const loadGpxTrack = useAppStore((s) => s.loadGpxTrack);
  const clearGpxTrack = useAppStore((s) => s.clearGpxTrack);

  useEffect(() => {
    void refreshGeotaggedPhotos();
  }, [refreshGeotaggedPhotos]);

  // Karte einmalig aufbauen.
  useEffect(() => {
    if (!containerRef.current || mapRef.current) return;
    const map = L.map(containerRef.current).setView([20, 0], 2);
    L.tileLayer("https://tile.openstreetmap.org/{z}/{x}/{y}.png", {
      attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>',
      maxZoom: 19,
    }).addTo(map);
    markersLayerRef.current = L.layerGroup().addTo(map);
    mapRef.current = map;

    map.on("click", (event: L.LeafletMouseEvent) => {
      void useAppStore.getState().setPhotoGpsFromMapClick(event.latlng.lat, event.latlng.lng);
    });

    return () => {
      map.remove();
      mapRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Marker + Reiserouten-Linie neu zeichnen, wenn sich die Fotoliste ändert.
  useEffect(() => {
    const map = mapRef.current;
    const layer = markersLayerRef.current;
    if (!map || !layer) return;
    layer.clearLayers();
    if (routeLayerRef.current) {
      routeLayerRef.current.remove();
      routeLayerRef.current = null;
    }

    if (geotaggedPhotos.length === 0) return;

    const routePoints: L.LatLngExpression[] = [];
    for (const photo of geotaggedPhotos) {
      if (photo.gps_lat === null || photo.gps_lon === null) continue;
      const latlng: L.LatLngExpression = [photo.gps_lat, photo.gps_lon];
      routePoints.push(latlng);
      const marker = L.marker(latlng, { icon: photo.id === selectedPhotoId ? SELECTED_ICON : PHOTO_ICON });
      marker.on("click", () => selectPhoto(photo.id));
      marker.bindPopup(popupHtmlFor(photo));
      marker.on("popupopen", () => {
        void reverseGeocodeLocation(photo.gps_lat as number, photo.gps_lon as number).then((location) => {
          marker.setPopupContent(popupHtmlFor(photo, location.name, location.country_code));
        });
      });
      layer.addLayer(marker);
    }

    if (routePoints.length >= 2) {
      routeLayerRef.current = L.polyline(routePoints, { color: "#3b82f6", weight: 2, dashArray: "4 6" }).addTo(map);
    }

    if (routePoints.length > 0) {
      map.fitBounds(L.latLngBounds(routePoints), { padding: [40, 40], maxZoom: 12 });
    }
  }, [geotaggedPhotos, selectedPhotoId, selectPhoto]);

  // GPX-Track als eigene, durchgezogene Linie überlagern.
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    if (gpxLayerRef.current) {
      gpxLayerRef.current.remove();
      gpxLayerRef.current = null;
    }
    if (!gpxTrack || gpxTrack.length === 0) return;
    const points: L.LatLngExpression[] = gpxTrack.map((p) => [p.lat, p.lon]);
    gpxLayerRef.current = L.polyline(points, { color: "#22c55e", weight: 3 }).addTo(map);
    map.fitBounds(L.latLngBounds(points), { padding: [40, 40] });
  }, [gpxTrack]);

  async function handleImportGpx() {
    const path = await pickFilePath("GPX-Tracklog", ["gpx"]);
    if (path) await loadGpxTrack(path);
  }

  return (
    <div className="relative flex-1 overflow-hidden">
      <div ref={containerRef} className="h-full w-full" />
      <div className="absolute right-3 top-3 z-[1000] flex flex-col gap-2 rounded border border-border bg-bg-raised p-2 text-xs shadow">
        <span className="text-text-secondary">{geotaggedPhotos.length} Foto{geotaggedPhotos.length === 1 ? "" : "s"} mit GPS</span>
        <button type="button" onClick={() => void handleImportGpx()} className="rounded border border-border px-2 py-1 hover:border-accent">
          GPX-Track importieren…
        </button>
        {gpxTrack && (
          <button type="button" onClick={clearGpxTrack} className="rounded border border-border px-2 py-1 hover:border-accent">
            GPX-Track entfernen ({gpxTrack.length} Punkte)
          </button>
        )}
        {selectedPhotoId && placingGpsForPhotoId !== selectedPhotoId && (
          <button type="button" onClick={() => startPlacingGps(selectedPhotoId)} className="rounded border border-border px-2 py-1 hover:border-accent">
            Standort für ausgewähltes Foto setzen…
          </button>
        )}
        {placingGpsForPhotoId && (
          <div className="flex flex-col gap-1">
            <span className="text-accent">Klick auf die Karte setzt den Standort</span>
            <button type="button" onClick={cancelPlacingGps} className="rounded border border-border px-2 py-1 hover:border-accent">
              Abbrechen
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
