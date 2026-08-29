import type { Page } from "@playwright/test";

/**
 * Simulierte `window.__TAURI_INTERNALS__`-Brücke für die Playwright-Tests.
 *
 * Siehe `DECISIONS.md` ADR-0010: Diese Tests laufen gegen den
 * Produktions-Build im normalen Chromium, nicht gegen die kompilierte
 * native App (das bräuchte `tauri-driver` + WebdriverIO). Damit das
 * Frontend trotzdem lauffähig ist, wird hier exakt der Teil der
 * `@tauri-apps/api`-Laufzeit nachgebaut, den `src/lib/tauri.ts`,
 * `src/lib/media.ts` und `src/hooks/useImportEvents.ts` tatsächlich
 * benutzen:
 *
 * - `invoke(cmd, args)` — ruft für die App-eigenen Commands
 *   (`select_folder`, `catalog_status`, `list_folders`,
 *   `list_photos_in_folder`, `import_folder`, `cancel_import`) fest
 *   hinterlegte, zur Laufzeit änderbare Fixture-Daten ab.
 * - `invoke('plugin:event|listen'/'plugin:event|unlisten', ...)` — das ist
 *   es, was `@tauri-apps/api/event`s `listen()` unter der Haube aufruft
 *   (siehe `node_modules/@tauri-apps/api/event.js`); hier nachgebaut,
 *   damit `useImportEvents` echte Listener registriert.
 * - `transformCallback(fn, once)` — registriert `fn` unter einer
 *   numerischen ID, exakt wie `core.js` es von den echten Tauri-Internals
 *   erwartet.
 * - `convertFileSrc(path, protocol)` — liefert statt einer echten
 *   `apx://…`-URL (die im Browser nicht aufgelöst werden kann) eine
 *   normale Same-Origin-URL, die der Test über `page.route()` mit einem
 *   winzigen, echten PNG beantwortet — so durchläuft `useImageBitmap`
 *   (fetch → blob → `createImageBitmap`) den echten Code, nicht nur eine
 *   Attrappe.
 *
 * Alles läuft als `page.addInitScript`, also im Browser-Kontext — die
 * Funktion darf deshalb nichts aus dem Node-Scope schließen.
 */
function installBridge(initialFixtures: Record<string, unknown>): void {
  interface CallbackEntry {
    fn: (arg: unknown) => void;
    once: boolean;
  }

  const w = window as unknown as Record<string, unknown>;

  w.__mockFixtures = {
    selectFolderResult: null as string | null,
    catalogStatus: { catalog_path: "mock-catalog.sqlite3", folder_count: 0, photo_count: 0 },
    folders: [] as unknown[],
    photosByFolder: {} as Record<string, unknown[]>,
    ...initialFixtures,
  };
  w.__mockInvokeLog = [] as Array<{ cmd: string; args: unknown }>;

  let nextCallbackId = 1;
  let nextListenerId = 1;
  const callbacks: Record<number, CallbackEntry> = {};
  const listenersByEvent: Record<string, number[]> = {};

  // Simuliertes `edit_history`/`edit_current` fürs Entwickeln-Panel (ab
  // Phase 2, siehe `crates/apx-catalog`s `repository::edits` und
  // `crates/apx-app/src/commands.rs`) — dieselbe lineare
  // Verlaufs-Semantik (neue Bearbeitung nach Rückgängig verwirft die
  // "Zukunft"), nur im Browser statt in SQLite.
  interface EditHistoryState {
    entries: Array<{ edl_json: string }>;
    currentIndex: number; // -1 = neutral (noch nie bearbeitet / bis zum Anfang zurück)
  }
  const editHistories: Record<string, EditHistoryState> = {};

  function historyPositionAt(history: EditHistoryState): unknown {
    if (history.currentIndex < 0) return { kind: "Neutral" };
    const entry = history.entries[history.currentIndex];
    return entry ? { kind: "At", edl_json: entry.edl_json } : { kind: "Neutral" };
  }

  w.__mockEmit = (eventName: string, payload: unknown) => {
    const ids = listenersByEvent[eventName] ?? [];
    for (const id of [...ids]) {
      const entry = callbacks[id];
      if (!entry) continue;
      entry.fn({ event: eventName, id, payload });
      if (entry.once) delete callbacks[id];
    }
  };

  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener(eventName: string, listenerId: number) {
      // Der Listener-Rückgabewert von 'plugin:event|listen' ist hier die
      // Handler-ID selbst (siehe invoke() unten) — genau die entfernen
      // wir wieder, analog zur echten Tauri-Runtime.
      listenersByEvent[eventName] = (listenersByEvent[eventName] ?? []).filter((id) => id !== listenerId);
    },
  };

  async function invoke(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
    (w.__mockInvokeLog as unknown[]).push({ cmd, args });
    const fixtures = w.__mockFixtures as {
      selectFolderResult: string | null;
      catalogStatus: unknown;
      folders: unknown[];
      photosByFolder: Record<string, unknown[]>;
    };

    switch (cmd) {
      case "plugin:event|listen": {
        const handlerId = args.handler as number;
        const eventName = args.event as string;
        listenersByEvent[eventName] = listenersByEvent[eventName] ?? [];
        listenersByEvent[eventName].push(handlerId);
        return nextListenerId++;
      }
      case "plugin:event|unlisten": {
        (w.__TAURI_EVENT_PLUGIN_INTERNALS__ as { unregisterListener: (e: string, id: number) => void }).unregisterListener(
          args.event as string,
          args.eventId as number,
        );
        return null;
      }
      case "select_folder":
        return fixtures.selectFolderResult;
      case "catalog_status":
        return fixtures.catalogStatus;
      case "list_folders":
        return fixtures.folders;
      case "list_photos_in_folder":
        return fixtures.photosByFolder[args.folderId as string] ?? [];
      case "import_folder":
        return null;
      case "cancel_import":
        return null;
      case "apply_develop_edit": {
        const photoId = args.photoId as string;
        const edlJson = args.edlJson as string;
        const history = (editHistories[photoId] ??= { entries: [], currentIndex: -1 });
        // Verwirft eine zuvor per Rückgängig erreichte "Zukunft" — siehe
        // `DECISIONS.md` ADR-0014, exakt wie das echte Backend.
        history.entries = history.entries.slice(0, history.currentIndex + 1);
        history.entries.push({ edl_json: edlJson });
        history.currentIndex = history.entries.length - 1;
        return null;
      }
      case "current_develop_edit": {
        const history = editHistories[args.photoId as string];
        return history ? historyPositionAt(history) : { kind: "Neutral" };
      }
      case "undo_develop_edit": {
        const history = editHistories[args.photoId as string];
        if (!history || history.currentIndex < 0) return null;
        history.currentIndex -= 1;
        return historyPositionAt(history);
      }
      case "redo_develop_edit": {
        const history = editHistories[args.photoId as string];
        if (!history || history.currentIndex + 1 >= history.entries.length) return null;
        history.currentIndex += 1;
        return historyPositionAt(history);
      }
      default:
        throw new Error(`Test-Stub: unbekannter invoke-Befehl "${cmd}"`);
    }
  }

  w.__TAURI_INTERNALS__ = {
    convertFileSrc: (path: string, protocol = "asset") =>
      `${window.location.origin}/__apx_mock_image__?src=${encodeURIComponent(`${protocol}:${path}`)}`,
    invoke: (cmd: string, args?: Record<string, unknown>) => invoke(cmd, args),
    transformCallback: (callback: (arg: unknown) => void, once = false) => {
      const id = nextCallbackId++;
      callbacks[id] = { fn: callback, once };
      return id;
    },
    unregisterCallback: (id: number) => {
      delete callbacks[id];
    },
  };
}

// Ein 2×2-PNG (transparent) als Antwort auf jede `__apx_mock_image__`-
// Anfrage — reicht aus, damit `createImageBitmap()` einen echten Treffer
// landet, ohne eine Bilddatei ins Repo aufzunehmen.
const TINY_PNG_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFUlEQVR42mNk+A8EDEwMo4ImDgYAAKn9BPz2j1CmAAAAAElFTkSuQmCC";

/**
 * Installiert die simulierte Tauri-Brücke für die gegebene Seite.
 *
 * `initialFixtures` wird schon *vor* dem ersten Laden der Seite gesetzt
 * (über `page.addInitScript`s Argument) — nötig für Tests, die nicht den
 * kompletten Import-Ablauf nachspielen, sondern direkt mit vorbefülltem
 * Katalogzustand starten wollen (z. B. der Virtualisierungs-Test), da
 * `App.tsx` `refreshFolders()` bereits beim Mounten aufruft, also noch
 * bevor ein nachträgliches `setMockFixtures()` greifen könnte.
 */
export async function installTauriMock(page: Page, initialFixtures: Record<string, unknown> = {}): Promise<void> {
  await page.route("**/__apx_mock_image__*", async (route) => {
    const src = new URL(route.request().url()).searchParams.get("src") ?? "";
    if (src.startsWith("apx:develop/")) {
      // Entwickeln-Route (ab Phase 2): kein Bildformat, sondern ein
      // 8-Byte-Breite/Höhe-Header + rohes RGBA8 (siehe
      // `crates/apx-app/src/protocol/mod.rs`, `DECISIONS.md` ADR-0016) —
      // hier ein einheitlich mittelgraues, undurchsichtiges 2×2-Bild.
      const width = 2;
      const height = 2;
      const header = Buffer.alloc(8);
      header.writeUInt32LE(width, 0);
      header.writeUInt32LE(height, 4);
      const pixels = Buffer.alloc(width * height * 4);
      for (let i = 0; i < pixels.length; i += 4) {
        pixels[i] = 128;
        pixels[i + 1] = 128;
        pixels[i + 2] = 128;
        pixels[i + 3] = 255;
      }
      await route.fulfill({
        status: 200,
        contentType: "application/x-apx-develop-rgba8",
        body: Buffer.concat([header, pixels]),
      });
      return;
    }

    await route.fulfill({
      status: 200,
      contentType: "image/png",
      body: Buffer.from(TINY_PNG_BASE64, "base64"),
    });
  });
  await page.addInitScript(installBridge, initialFixtures);
}

/** Ersetzt (per `Object.assign`) einen Teil der Invoke-Fixture-Daten. */
export async function setMockFixtures(page: Page, partial: Record<string, unknown>): Promise<void> {
  await page.evaluate((data) => {
    Object.assign((window as unknown as Record<string, unknown>).__mockFixtures as object, data);
  }, partial);
}

/** Simuliert ein vom Backend emittiertes Tauri-Event (siehe `useImportEvents`). */
export async function emitMockEvent(page: Page, eventName: string, payload: unknown): Promise<void> {
  await page.evaluate(
    ([name, data]) => {
      (window as unknown as { __mockEmit: (n: string, p: unknown) => void }).__mockEmit(name, data);
    },
    [eventName, payload] as const,
  );
}

/** Liest das Protokoll aller bisherigen `invoke()`-Aufrufe aus (zum Assertieren). */
export async function getMockInvokeLog(page: Page): Promise<Array<{ cmd: string; args: unknown }>> {
  return page.evaluate(() => (window as unknown as Record<string, unknown>).__mockInvokeLog as Array<{ cmd: string; args: unknown }>);
}
