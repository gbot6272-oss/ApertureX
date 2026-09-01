import { useAppStore } from "../store";
import { de } from "./locales/de";
import { en } from "./locales/en";

/**
 * Lokalisierung DE/EN (Phase 10 Schritt 8, siehe `FEATURES.md` §5 —
 * SPEC.md verlangt explizit "Lokalisierung (Deutsch und Englisch)").
 * **Bewusst kein neues npm-Paket** (`react-i18next` wäre eine schwere
 * Abhängigkeit für eine reine Key-Lookup-Funktion) — flache
 * Wörterbücher + eine `t()`-Funktion, dieselbe Größenordnung wie die
 * anderen schlanken Eigenlösungen in diesem Projekt.
 *
 * Deutsch ist die Schlüssel-/Ausgangssprache: `de.ts`s Werte sind
 * character-für-character identisch mit den vormals hartkodierten
 * deutschen Strings der jeweiligen Komponente — das hält jeden
 * bestehenden `getByText`/`getByRole(..., { name })`-e2e-Testpfad
 * gültig, weil `uiSettings.locale` standardmäßig `"de"` ist
 * (`apx_core::settings::UiSettings::default()`) und alle Tests mit
 * diesem Standard laufen.
 *
 * **Ehrlich begrenzt** (siehe DECISIONS.md ADR-0037): übersetzt sind die
 * durchgängig sichtbare Navigations-/Rahmen-UI (Header, Seitenleiste,
 * Einstellungen, Cheatsheet, Metadaten-/Presets-Panel-Überschriften) —
 * nicht die ca. 20 Dialog-Komponenten (Export/Druck/Diashow/Buch/Web/
 * Vorlagen/Organisieren/Stacking/Skript/Kollaboration/Tethering/
 * Metadaten-Editor/Statistik), die den Großteil der ~10.700
 * Frontend-Zeilen und praktisch alle exakten deutschen
 * e2e-Testtreffer tragen. Deren Übersetzung bleibt eine offene
 * Ausbaustufe für eine spätere Runde, nicht stillschweigend
 * unvollständig behauptet.
 */

export type Locale = "de" | "en";
export type TranslationKey = keyof typeof de;

const DICTIONARIES: Record<Locale, Record<TranslationKey, string>> = { de, en };

export function translate(locale: Locale, key: TranslationKey, vars?: Record<string, string | number>): string {
  const dict = DICTIONARIES[locale] ?? DICTIONARIES.de;
  let text = dict[key] ?? DICTIONARIES.de[key] ?? key;
  if (vars) {
    for (const [name, value] of Object.entries(vars)) {
      text = text.replaceAll(`{${name}}`, String(value));
    }
  }
  return text;
}

/** Liest die aktuelle Sprache aus dem Store (Fallback "de", solange
 * `uiSettings` noch nicht geladen ist — derselbe Wert wie
 * `UiSettings::default()` im Backend, kein sichtbarer Sprachwechsel beim
 * Nachladen). */
export function useLocale(): Locale {
  return useAppStore((s) => (s.uiSettings?.locale === "en" ? "en" : "de"));
}

export function useT(): (key: TranslationKey, vars?: Record<string, string | number>) => string {
  const locale = useLocale();
  return (key, vars) => translate(locale, key, vars);
}
