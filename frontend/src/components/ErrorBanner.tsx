import { useAppStore } from "../store";

export function ErrorBanner() {
  const catalogError = useAppStore((s) => s.catalogError);
  const importErrors = useAppStore((s) => s.importErrors);

  if (!catalogError && importErrors.length === 0) return null;

  return (
    <div className="max-h-32 shrink-0 overflow-y-auto border-b border-danger/40 bg-danger/10 px-4 py-2 text-xs text-danger">
      {catalogError && <div>{catalogError}</div>}
      {importErrors.map((line) => (
        <div key={line}>{line}</div>
      ))}
    </div>
  );
}
