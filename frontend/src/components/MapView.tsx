import L from "leaflet";
import "leaflet/dist/leaflet.css";
import { useEffect, useRef, useState } from "react";

import { GlobeView } from "./GlobeView";
import { createPhotoHeatLayer, type PhotoHeatLayer } from "../lib/leafletHeatmap";
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

/** Ab diesem Leaflet-Zoomlevel werden statt der Dichte-Heatmap
 * einzelne Foto-Pins gezeigt — bewusst hoch angesetzt ("sehr großer
 * Zoom", Nutzervorgabe, siehe `DECISIONS.md` ADR-0044): 10 entspricht
 * bei Web-Mercator-Kacheln ungefähr Stadt-/Bezirksebene, deutlich
 * näher als die üblichen Land-/Regionsebene-Zoomstufen, auf denen die
 * Heatmap noch aussagekräftiger als hunderte überlappende Pins ist. */
const PIN_ZOOM_THRESHOLD = 10;

/** Dunkle, kostenlose Kachel-Basiskarte ohne API-Schlüssel (CARTOs
 * "Dark Matter", CC-BY-3.0-Kacheln über OpenStreetMap-Daten) statt der
 * bisherigen hellen Standard-OSM-Kacheln — passend zum "technisch/
 * professionell/dunkel"-Anspruch dieser Erweiterung (siehe
 * `DECISIONS.md` ADR-0044) und zum Globus, der dieselbe dunkle
 * Grundstimmung hat. */
const DARK_TILE_URL = "https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png";
const DARK_TILE_ATTRIBUTION =
  '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors &copy; <a href="https://carto.com/attributions">CARTO</a>';

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function popupHtmlFor(photo: PhotoDto, locationName?: string, countryCode?: string): string {
  const location = locationName ? `${locationName}${countryCode ? `, ${countryCode}` : ""}` : "Ort wird ermittelt…";
  return `<strong>${escapeHtml(photo.filename)}</strong><br/>${escapeHtml(location)}`;
}

function readHeatColors() {
  const style = getComputedStyle(document.documentElement);
  const read = (name: string, fallback: string) => style.getPropertyValue(name).trim() || fallback;
  return {
    cool: read("--color-accent", "#5b9bd5"),
    mid: read("--color-accent", "#5b9bd5"),
    hot: read("--color-danger", "#e07a5f"),
  };
}

/**
 * Kartenansicht (Phase 8 Schritt 7, erweitert siehe `DECISIONS.md`
 * ADR-0044). Drei Ebenen statt nur einer flachen Kachelkarte:
 *
 * 1. **Globus** (`GlobeView.tsx`) — der Standard-Einstieg, "kleinste
 *    Ebene": eine selbst gerenderte drehbare 3D-Kugel mit einer
 *    Foto-Dichte-Heatmap auf der Oberfläche, keine einzelnen Pins.
 * 2. **Flache Karte, Heatmap-Zoom** — nach "Zur Karte" (oder wenn die
 *    Karte weit herausgezoomt ist): dieselbe Dichte-Heatmap, jetzt auf
 *    einer echten Leaflet-Kachelkarte (dunkle CARTO-Kacheln statt der
 *    ursprünglichen hellen OSM-Kacheln).
 * 3. **Flache Karte, Pin-Zoom** (`PIN_ZOOM_THRESHOLD`) — erst bei sehr
 *    großem Zoom lösen sich einzelne Foto-Pins aus der Heatmap
 *    (dasselbe Marker-/Popup-/Reiserouten-Verhalten wie zuvor).
 *
 * Bewusst mit Leaflet direkt statt `react-leaflet` (kein zusätzlicher
 * Adapter-Layer nötig, ein `useEffect` mit einer imperativen Leaflet-
 * Instanz reicht für diesen einen View) und mit einer selbst
 * implementierten Heatmap statt einer neuen `leaflet.heat`-Abhängigkeit
 * (siehe `lib/leafletHeatmap.ts`s Moduldoku).
 */
export function MapView() {
  const containerRef = useRef<HTMLDivElement>(null);
  const mapRef = useRef<L.Map | null>(null);
  const markersLayerRef = useRef<L.LayerGroup | null>(null);
  const heatLayerRef = useRef<PhotoHeatLayer | null>(null);
  const routeLayerRef = useRef<L.Polyline | null>(null);
  const gpxLayerRef = useRef<L.Polyline | null>(null);

  const [mapMode, setMapMode] = useState<"globe" | "map">("globe");
  const [showPins, setShowPins] = useState(false);
  const [zoomLevel, setZoomLevel] = useState(2);

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

  // Karte einmalig aufbauen — erst, sobald der Modus auf "map"
  // wechselt (der Globus rendert bis dahin sein eigenes Canvas; ein
  // leeres Leaflet-Div im Hintergrund aufzubauen wäre reine
  // Verschwendung, siehe Render-Zweig unten).
  useEffect(() => {
    if (mapMode !== "map" || !containerRef.current || mapRef.current) return;
    const map = L.map(containerRef.current).setView([20, 0], 2);
    L.tileLayer(DARK_TILE_URL, {
      attribution: DARK_TILE_ATTRIBUTION,
      maxZoom: 19,
    }).addTo(map);
    markersLayerRef.current = L.layerGroup().addTo(map);
    heatLayerRef.current = createPhotoHeatLayer([], readHeatColors);
    heatLayerRef.current.addTo(map);
    mapRef.current = map;
    setShowPins(map.getZoom() >= PIN_ZOOM_THRESHOLD);
    setZoomLevel(map.getZoom());

    map.on("click", (event: L.LeafletMouseEvent) => {
      void useAppStore.getState().setPhotoGpsFromMapClick(event.latlng.lat, event.latlng.lng);
    });
    map.on("zoomend", () => {
      setShowPins(map.getZoom() >= PIN_ZOOM_THRESHOLD);
      setZoomLevel(map.getZoom());
    });

    return () => {
      map.remove();
      mapRef.current = null;
      markersLayerRef.current = null;
      heatLayerRef.current = null;
    };
  }, [mapMode]);

  // Heatmap-Punkte aktuell halten (unabhängig vom Pin-/Heatmap-
  // Sichtbarkeitszustand — die Heatmap-Ebene bleibt immer mit den
  // aktuellen Daten bestückt, nur ihre Sichtbarkeit wechselt).
  useEffect(() => {
    const points = geotaggedPhotos
      .filter((p) => p.gps_lat !== null && p.gps_lon !== null)
      .map((p) => ({ lat: p.gps_lat as number, lon: p.gps_lon as number }));
    heatLayerRef.current?.setPoints(points);
    // `mapMode` gehört bewusst mit in die Abhängigkeitsliste: die
    // Karte (und damit `heatLayerRef.current`) entsteht erst beim
    // Wechsel von Globus zu Karte — ohne diese Abhängigkeit würde ein
    // bereits vorher geladener `geotaggedPhotos`-Stand nie in die neu
    // erzeugte Heatmap-Ebene übertragen (der Effekt liefe nur beim
    // *nächsten* Foto-Datenwechsel erneut, nicht beim Moduswechsel).
  }, [geotaggedPhotos, mapMode]);

  // Heatmap/Pin-Ebene je nach Zoomstufe ein-/ausblenden.
  useEffect(() => {
    const map = mapRef.current;
    const heat = heatLayerRef.current;
    const markers = markersLayerRef.current;
    if (!map || !heat || !markers) return;
    if (showPins) {
      if (map.hasLayer(heat)) map.removeLayer(heat);
      if (!map.hasLayer(markers)) markers.addTo(map);
    } else {
      if (map.hasLayer(markers)) map.removeLayer(markers);
      if (!map.hasLayer(heat)) heat.addTo(map);
    }
  }, [showPins, mapMode]);

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
  }, [geotaggedPhotos, selectedPhotoId, selectPhoto]);

  // Beim ersten Wechsel in den Kartenmodus einmalig auf die
  // geotaggten Fotos einrahmen (nur einmal — sonst würde jede
  // Foto-Listenänderung während der Nutzung die aktuelle Zoom-/Pan-
  // Position wegreißen).
  const didFitBoundsRef = useRef(false);
  useEffect(() => {
    const map = mapRef.current;
    if (!map || mapMode !== "map" || didFitBoundsRef.current) return;
    const points: L.LatLngExpression[] = geotaggedPhotos
      .filter((p) => p.gps_lat !== null && p.gps_lon !== null)
      .map((p) => [p.gps_lat as number, p.gps_lon as number]);
    if (points.length === 0) return;
    map.fitBounds(L.latLngBounds(points), { padding: [40, 40], maxZoom: 12 });
    didFitBoundsRef.current = true;
  }, [geotaggedPhotos, mapMode]);

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
  }, [gpxTrack, mapMode]);

  async function handleImportGpx() {
    const path = await pickFilePath("GPX-Tracklog", ["gpx"]);
    if (path) await loadGpxTrack(path);
  }

  if (mapMode === "globe") {
    return <GlobeView photos={geotaggedPhotos} onEnterMap={() => setMapMode("map")} />;
  }

  return (
    <div className="relative flex-1 overflow-hidden">
      <div ref={containerRef} className="h-full w-full" />
      <div className="absolute right-3 top-3 z-[1000] flex flex-col gap-2 rounded border border-border bg-bg-raised p-2 text-xs shadow">
        <div className="flex items-center justify-between gap-3">
          <span className="text-text-secondary">{geotaggedPhotos.length} Foto{geotaggedPhotos.length === 1 ? "" : "s"} mit GPS</span>
          <button type="button" onClick={() => setMapMode("globe")} className="rounded border border-border px-2 py-0.5 hover:border-accent">
            🌐 Globus
          </button>
        </div>
        <span className="text-text-muted">{showPins ? "einzelne Fotos" : "Dichte-Heatmap"} (Zoom {zoomLevel})</span>
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
