import { useAppStore } from "../store";

export function ErrorBanner() {
  const catalogError = useAppStore((s) => s.catalogError);
  const importErrors = useAppStore((s) => s.importErrors);

  if (!catalogError && importErrors.length === 0) return null;

  // Phase 25 Nachtrag III: die Kopfzeile schwebt seit `Header.tsx`s
  // aktuellem Kommentar jetzt als `fixed`-Überlagerung außerhalb des
  // Dokumentflusses — dieses Banner ist dadurch potenziell das erste
  // Flusselement direkt am oberen Rand und braucht deshalb selbst
  // `pt-12` (48px), um nicht unter der Kopfzeile zu verschwinden. Nur
  // hier statt in `App.tsx` gesetzt, weil dieses Element ohnehin oft
  // `null` rendert (kein Geisterabstand, wenn kein Fehler ansteht).
  return (
    <div className="max-h-32 shrink-0 overflow-y-auto border-b border-danger/40 bg-danger/10 px-4 pt-12 pb-2 text-xs text-danger">
      {catalogError && <div>{catalogError}</div>}
      {importErrors.map((line) => (
        <div key={line}>{line}</div>
      ))}
    </div>
  );
}
