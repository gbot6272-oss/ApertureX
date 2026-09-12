import { useCallback, useEffect, useRef } from "react";

import type { CueName, PackName, PlayOptions, UISFXPlayer } from "uisfx";
import { createUISFX, packNames } from "uisfx";

import { useAppStore } from "../store";

/**
 * UI-Sounds (Phase 19, siehe `DECISIONS.md` ADR-0047) — dünner Wrapper um
 * `uisfx` (MIT-Code + CC0-Audio, siehe `THIRD_PARTY.md`). Bewusst EIN
 * modulweiter Player-Singleton statt eines Players pro Komponente: `uisfx`
 * begrenzt Polyphonie/Cooldowns global, das funktioniert nur mit genau
 * einer Instanz für die ganze App (dieselbe Überlegung wie bei
 * `lib/motion.ts`s zentralen Bewegungs-Tokens).
 *
 * Persistenz läuft bewusst NICHT über `uisfx`s eigenes
 * `preferences`/`localStorage`-Feature, sondern über das bestehende
 * `uiSettings`/Rust-Settings-TOML derselben App (`sound_enabled`/
 * `sound_volume_percent`/`sound_pack`, siehe `lib/tauri.ts`) — sonst gäbe
 * es zwei parallele, potenziell widersprüchliche Speicherorte für
 * dieselbe Einstellung.
 */

const DEFAULT_PACK: PackName = "glass";

function isKnownPack(value: string): value is PackName {
  return (packNames as readonly string[]).includes(value);
}

/** Für die Einstellungen-UI (`SettingsDialog.tsx`) — Reihenfolge wie in
 * `uisfx`s eigenem README, deutsche Kurzbeschreibung je Klangwelt. */
export const SOUND_PACKS: Array<{ id: PackName; label: string; description: string }> = [
  { id: "minimal", label: "Minimal", description: "Trocken, präzise, fast unauffällig" },
  { id: "soft", label: "Soft", description: "Rund, warm, zurückhaltend" },
  { id: "glass", label: "Glass", description: "Hell, klar, hochwertig — für Medien-/Kreativ-Werkzeuge" },
  { id: "studio", label: "Studio", description: "Präzise mit warmer Zurückhaltung — für Foto/Video/Audio" },
  { id: "cinematic", label: "Cinematic", description: "Satte Wirkung, gut für Start/Export-Momente" },
  { id: "scifi", label: "Sci-Fi", description: "Klare, futuristische Klänge" },
  { id: "mechanical", label: "Mechanical", description: "Schalter/Relais, feste Rastpunkte" },
  { id: "dreamy", label: "Dreamy", description: "Luftig, sanft ausklingend" },
  { id: "organic", label: "Organic", description: "Holz, Wasser, ruhig" },
  { id: "zen", label: "Zen", description: "Papier, Pinsel, leise Klangschalen" },
  { id: "arcade", label: "Arcade", description: "Verspielt, deutlich hörbar" },
  { id: "rubber", label: "Rubber", description: "Taktil, elastisch, freundlich" },
];

let player: UISFXPlayer | null = null;

function getPlayer(): UISFXPlayer {
  if (!player) {
    // Kein `preferences`-Objekt übergeben (s.o., Moduldoku) — Pack/
    // Lautstärke/Aktiv-Status werden stattdessen per `applySoundSettings`
    // unten aus `uiSettings` gesetzt, sobald die Einstellungen geladen
    // sind. `AudioContext` wird laut `uisfx`-Dokumentation erst bei
    // Bedarf (Web-Audio "lazy") angelegt — dieser Aufruf selbst führt in
    // keiner Umgebung (auch nicht in Playwright/jsdom ohne Audiogerät)
    // zu einem Fehler.
    player = createUISFX({ pack: DEFAULT_PACK, enabled: false, volume: 0.85 });
  }
  return player;
}

/** Aus `App.tsx` bei jeder `uiSettings`-Änderung aufgerufen — hält den
 * Player-Singleton mit den gespeicherten Einstellungen synchron. */
export function applySoundSettings(settings: {
  sound_enabled: boolean;
  sound_volume_percent: number;
  sound_pack: string;
}): void {
  const p = getPlayer();
  p.setEnabled(settings.sound_enabled);
  p.setVolume(Math.max(0, Math.min(100, settings.sound_volume_percent)) / 100);
  p.setPack(isKnownPack(settings.sound_pack) ? settings.sound_pack : DEFAULT_PACK);
}

/**
 * Browser-Autoplay-Policy: `AudioContext` darf erst nach einer echten
 * (vertrauenswürdigen) Zeiger-/Tastatur-Interaktion starten — `App.tsx`
 * ruft das einmalig beim ersten `pointerdown`/`keydown` auf. Mehrfaches
 * Aufrufen ist laut `uisfx`-API sicher (idempotent).
 */
export function unlockSound(): void {
  void getPlayer()
    .unlock()
    .catch(() => {
      // Web Audio kann in seltenen Umgebungen (z. B. ohne Audiogerät)
      // ablehnen — ein UI-Sound ist rein dekorativ, daher hier bewusst
      // stumm ignoriert statt eine sichtbare Fehlermeldung auszulösen.
    });
}

/** Für `SettingsDialog.tsx`s Klangwelt-Auswahl: setzt den Player sofort
 * auf ein Pack und spielt einen Beispiel-Cue — unabhängig vom
 * `sound_enabled`-Status, damit man eine Klangwelt auch bei
 * ausgeschalteten Sounds kurz anhören kann, bevor man sie aktiviert. */
export function previewPack(pack: PackName): void {
  try {
    const p = getPlayer();
    const wasEnabled = p.isEnabled();
    p.setPack(pack);
    if (!wasEnabled) p.setEnabled(true);
    p.play("success");
    if (!wasEnabled) p.setEnabled(false);
  } catch {
    // Siehe `unlockSound` oben.
  }
}

/**
 * Spielt einen semantischen Cue ab, falls Sounds aktiviert sind. Wirft nie
 * — ein fehlgeschlagener UI-Sound darf niemals eine echte Aktion (Klick,
 * Export, Speichern) unterbrechen.
 */
export function playCue(cue: CueName, options?: PlayOptions): void {
  try {
    getPlayer().play(cue, options);
  } catch {
    // Siehe `unlockSound` oben — Sound ist rein dekorativ.
  }
}

/** Bequemer Hook für Komponenten: `const playSound = useSound(); ...
 * playSound("success")`. Liest keinen Store-Zustand (der Player-Singleton
 * kennt seinen Aktiv-Status bereits selbst über `applySoundSettings`),
 * daher löst der Hook selbst keine Re-Renders aus. */
export function useSound(): (cue: CueName, options?: PlayOptions) => void {
  return useCallback((cue: CueName, options?: PlayOptions) => playCue(cue, options), []);
}

/**
 * Einmaliges App-weites `unlock()` an die erste vertrauenswürdige
 * Zeiger-/Tastatur-Interaktion binden (siehe `unlockSound` oben) — als
 * Hook statt direkt in `App.tsx`s Haupt-`useEffect`, damit der Zweck an
 * der Aufrufstelle benannt bleibt.
 */
export function useUnlockSoundOnFirstInteraction(): void {
  const unlockedRef = useRef(false);
  useEffect(() => {
    function handleFirstInteraction() {
      if (unlockedRef.current) return;
      unlockedRef.current = true;
      unlockSound();
      window.removeEventListener("pointerdown", handleFirstInteraction);
      window.removeEventListener("keydown", handleFirstInteraction);
    }
    window.addEventListener("pointerdown", handleFirstInteraction);
    window.addEventListener("keydown", handleFirstInteraction);
    return () => {
      window.removeEventListener("pointerdown", handleFirstInteraction);
      window.removeEventListener("keydown", handleFirstInteraction);
    };
  }, []);
}

/** Für Komponenten, die direkt (ohne Store-Abo) wissen müssen, ob Sounds
 * gerade aktiv sind, z. B. um eine sichtbare Rückmeldung nicht zu
 * verdoppeln. */
export function useSoundEnabled(): boolean {
  return useAppStore((s) => s.uiSettings?.sound_enabled ?? false);
}

/**
 * App-weiter "expand"/"collapse"-Sound für JEDES `<details
 * className="apx-collapsible">` (die Akkordeon-Abschnitte in
 * `DevelopPanel.tsx`/`MasksPanel.tsx`, siehe `index.css`s
 * `.apx-collapsible`-Regel) — ohne die ~36 Stellen einzeln anfassen zu
 * müssen. Das native `toggle`-Ereignis von `<details>` bubbelt NICHT,
 * ein einzelner delegierter Listener auf `document` erreicht es aber
 * trotzdem in der Capture-Phase (die läuft unabhängig vom Bubbling von
 * der Wurzel zum Ziel) — deshalb `addEventListener(..., true)` statt
 * eines gewöhnlichen (bubbelnden) Listeners. Einmalig aus `App.tsx`
 * aufgerufen.
 */
export function useAccordionSounds(): void {
  useEffect(() => {
    function handleToggle(event: Event) {
      const target = event.target as HTMLElement | null;
      if (!(target instanceof HTMLDetailsElement)) return;
      if (!target.classList.contains("apx-collapsible")) return;
      playCue(target.open ? "expand" : "collapse");
    }
    document.addEventListener("toggle", handleToggle, true);
    return () => document.removeEventListener("toggle", handleToggle, true);
  }, []);
}

/**
 * App-weites, leises Tastenanschlag-Geräusch (`uisfx`s dediziertem
 * `"press"`-Cue, siehe dessen eigene Kategorie "input" — genau für
 * diesen Zweck gedacht: ein kurzes, ruhiges Feedback beim physischen
 * Drücken, unabhängig von jedem semantischen Ergebnis-Sound) für JEDEN
 * `<button>`/`role="button"` app-weit (Phase 20, siehe DECISIONS.md
 * ADR-0048-Nachtrag) — Nutzer-Rückmeldung: "kein Hover, kein gar nix",
 * echte `<button>`-Elemente hatten trotz hunderter Stellen im Code
 * bislang **keinen einzigen** eigenen Sound, nur die wenigen Stellen,
 * die `playCue` explizit im eigenen `onClick` aufrufen (z. B.
 * `TabBar`/`GridView`s "select"). Ein einzelner delegierter
 * `pointerdown`-Listener auf `document` (Capture-Phase wie
 * `useAccordionSounds` oben) statt hunderter Einzelstellen — läuft
 * bewusst NEBEN einem eventuell vorhandenen semantischen Sound der
 * Komponente selbst (der feuert typischerweise erst beim `click`,
 * also einen Wimpernschlag später): ein kurzer, leiser "Anschlag"-Ton
 * beim Herunterdrücken plus ein spezifischerer Ton beim eigentlichen
 * Ergebnis ist dasselbe Schichtungsprinzip wie bei einer physischen
 * Taste (Anschlaggeräusch) plus einem Erfolgs-/Auswahl-Ton danach —
 * kein Widerspruch, keine Verdopplung derselben Bedeutung.
 */
export function useButtonPressSounds(): void {
  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      if (event.button !== 0) return;
      const target = event.target as HTMLElement | null;
      if (!target) return;
      const control = target.closest<HTMLElement>('button, [role="button"]');
      if (!control) return;
      if (control instanceof HTMLButtonElement && control.disabled) return;
      if (control.getAttribute("aria-disabled") === "true") return;
      playCue("press");
    }
    document.addEventListener("pointerdown", handlePointerDown, true);
    return () => document.removeEventListener("pointerdown", handlePointerDown, true);
  }, []);
}
