import { useEffect, useState } from "react";
import type { ClipboardEntry } from "../../types";
import {
  clipboardClear,
  clipboardCopy,
  clipboardDelete,
  clipboardList,
  clipboardPaste,
} from "./clipboard";

const REFRESH_MS = 1000;

function sameEntries(
  left: ClipboardEntry[],
  right: ClipboardEntry[],
): boolean {
  return (
    left.length === right.length &&
    left.every(
      (entry, index) =>
        entry.id === right[index].id &&
        entry.text === right[index].text &&
        entry.timestamp === right[index].timestamp,
    )
  );
}

export function useClipboard(resetKey: number, active: boolean) {
  const [entries, setEntries] = useState<ClipboardEntry[]>([]);
  const [loaded, setLoaded] = useState(false);

  const load = async () => {
    try {
      const next = await clipboardList();
      setEntries((previous) => (sameEntries(previous, next) ? previous : next));
    } catch {
      // Keep the previous history if the backend is unavailable.
    } finally {
      setLoaded(true);
    }
  };

  useEffect(() => {
    void load();
  }, [resetKey]);

  useEffect(() => {
    if (!active) return;

    const timer = window.setInterval(() => {
      void load();
    }, REFRESH_MS);

    return () => window.clearInterval(timer);
  }, [active, resetKey]);

  const copy = (id: string) => clipboardCopy(id);
  const paste = (id: string) => clipboardPaste(id);

  const remove = (id: string) => {
    setEntries((entries) => entries.filter((entry) => entry.id !== id));
    return clipboardDelete(id);
  };

  const clear = async () => {
    await clipboardClear();
    setEntries([]);
  };

  return { entries, loaded, copy, paste, remove, clear };
}
