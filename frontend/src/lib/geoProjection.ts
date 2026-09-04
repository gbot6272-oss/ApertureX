/**
 * Orthographische Kugel-Projektion für den Globus (siehe
 * `DECISIONS.md` ADR-0044) — dieselbe Mathematik, mit der z. B.
 * `d3-geo`s `geoOrthographic()` einen "echten Globus"-Look erzeugt,
 * hier selbst implementiert statt einer neuen Laufzeit-Abhängigkeit
 * (dieselbe Linie wie die selbst implementierte `.cube`-LUT-Engine in
 * Phase 16 oder das selbst implementierte Seam Carving in Phase 15 —
 * ein gut dokumentierter, kleiner Algorithmus, kein Grund für ein
 * externes Paket). Reine Funktionen, keine DOM-Abhängigkeit — leicht
 * unit-testbar (siehe `geoProjection.test.ts`).
 *
 * Konvention: `lon`/`lat` in Grad (WGS84-Konvention wie überall sonst
 * im Projekt, siehe `PhotoDto.gps_lat/gps_lon`), Rotation ebenfalls in
 * Grad. Die Kugel hat Einheitsradius; der Aufrufer skaliert auf
 * Bildschirm-Pixel.
 */

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

/** Blickwinkel auf den Globus — `yaw` dreht um die vertikale Achse
 * (Ost/West), `pitch` neigt die Polachse (Nord/Süd). Beide in Grad. */
export interface GlobeRotation {
  yaw: number;
  pitch: number;
}

const DEG2RAD = Math.PI / 180;

/** WGS84-Länge/Breite → Punkt auf der Einheitskugel. `z` zeigt zum
 * Betrachter bei Rotation `{yaw: 0, pitch: 0}` und `lon = lat = 0`. */
export function latLonToUnitSphere(lat: number, lon: number): Vec3 {
  const latRad = lat * DEG2RAD;
  const lonRad = lon * DEG2RAD;
  const cosLat = Math.cos(latRad);
  return {
    x: cosLat * Math.sin(lonRad),
    y: Math.sin(latRad),
    z: cosLat * Math.cos(lonRad),
  };
}

/** Dreht einen Kugelpunkt um `rotation` — erst um die Y-Achse (Yaw,
 * Ost/West-Blickrichtung), dann um die X-Achse (Pitch, Nord/Süd-
 * Neigung). Reihenfolge ist bewusst so gewählt, dass ein reines
 * Yaw-Dragging den Äquator immer horizontal in der Bildmitte hält. */
export function rotateSphere(point: Vec3, rotation: GlobeRotation): Vec3 {
  const yawRad = rotation.yaw * DEG2RAD;
  const pitchRad = rotation.pitch * DEG2RAD;

  // Yaw: Rotation um die Y-Achse.
  const cosYaw = Math.cos(yawRad);
  const sinYaw = Math.sin(yawRad);
  const x1 = point.x * cosYaw + point.z * sinYaw;
  const z1 = -point.x * sinYaw + point.z * cosYaw;
  const y1 = point.y;

  // Pitch: Rotation um die X-Achse.
  const cosPitch = Math.cos(pitchRad);
  const sinPitch = Math.sin(pitchRad);
  const y2 = y1 * cosPitch - z1 * sinPitch;
  const z2 = y1 * sinPitch + z1 * cosPitch;

  return { x: x1, y: y2, z: z2 };
}

export interface ProjectedPoint {
  /** Bildschirm-Koordinaten relativ zur Kugelmitte, in Einheiten des
   * übergebenen `radius` (noch nicht um das Canvas-Zentrum verschoben
   * — das macht der Aufrufer beim Zeichnen). */
   x: number;
   y: number;
  /** `true`, wenn der Punkt auf der dem Betrachter zugewandten
   * Hemisphäre liegt (`z >= 0` nach Rotation) — die Rückseite der
   * Kugel wird nicht gezeichnet. */
  visible: boolean;
}

/** Orthographische Projektion (Parallelprojektion entlang der
 * Z-Achse, kein Fluchtpunkt — das ist exakt der Effekt, der einen
 * Foto-Globus wie eine reale, aus der Ferne betrachtete Kugel statt
 * einer perspektivisch verzerrten 3D-Szene aussehen lässt) eines
 * bereits rotierten Kugelpunkts auf die Bildebene. */
export function orthographicProject(rotated: Vec3, radius: number): ProjectedPoint {
  return {
    x: rotated.x * radius,
    // Bildschirm-Y wächst nach unten, Kugel-Y (Breitengrad) nach
    // Norden — deshalb hier gespiegelt.
    y: -rotated.y * radius,
    visible: rotated.z >= 0,
  };
}

/** Bequemlichkeitsfunktion: Länge/Breite direkt zu einem projizierten
 * Bildschirmpunkt (relativ zur Kugelmitte) — die drei Schritte oben in
 * einem Aufruf, für den häufigsten Fall (Marker/Heatmap-Zellen). */
export function projectLatLon(lat: number, lon: number, rotation: GlobeRotation, radius: number): ProjectedPoint {
  const rotated = rotateSphere(latLonToUnitSphere(lat, lon), rotation);
  return orthographicProject(rotated, radius);
}

/** Wie stark ein Punkt der Kamera zugewandt ist (1 = Bildmitte/Pol
 * direkt zum Betrachter, 0 = genau am sichtbaren Rand) — genutzt, um
 * Landmasse/Heatmap zum Kugelrand hin sanft abzudunkeln (Beleuchtungs-
 * Näherung, dieselbe "Lambert-artige" Idee wie ein einfacher
 * Kugel-Shader, hier aber nur ein Skalar statt echter Normalen-
 * Beleuchtung). */
export function limbDarkening(rotatedZ: number): number {
  return Math.max(0, rotatedZ);
}
